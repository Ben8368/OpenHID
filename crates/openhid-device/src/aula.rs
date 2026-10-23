//! F87 PRO wireless configuration protocol.
//!
//! Frames were verified on a 3554:FA09 receiver. Unknown configuration bytes
//! are preserved. Only independently verified controls are exposed.

use std::time::Duration;

use openhid_device_registry::aula::matches_control;
use thiserror::Error;

use crate::backend::{BackendError, HidBackend, NodeInfo, RawSession};

mod backup;
mod battery;
mod lighting;
mod macros;
mod matrix;
mod palette;
pub mod per_key;
mod transaction;
mod wired;
pub use battery::read_selected_state;
pub(crate) use battery::read_state;
pub use lighting::{
    AulaColorModeChange, AulaColorModeState, LightingChange, LightingMode, LightingState,
    update_lighting,
};
pub use macros::{
    WirelessMacroPrefix, WirelessMacroTableCandidate, read_selected_macro_prefix,
    read_selected_macro_table_candidate,
};
pub use matrix::{
    KeymapPlanError, KeymapWriteError, MatrixWritePlan, WirelessMatrix, is_matrix_write_ack,
    plan_keymap_write, read_selected_base_matrix, read_selected_keymap,
    read_selected_keymap_with_oem_carrier, read_selected_matrix,
    read_selected_matrix_with_oem_carrier, update_selected_keymap,
    update_selected_keymap_if_unchanged,
};
pub use openhid_core::hid::aula::{
    AulaBattery, AulaBatteryPowerSource, AulaChange, AulaDebounce, AulaPerKeyRgb, AulaRgb,
    AulaSideLightEffect, AulaState, SleepTimeout,
};
pub use per_key::{PerKeyRgbError, apply_selected_per_key_rgb};
pub use transaction::{ConfigChange, ConfigError, ConfigResult, update_config};
pub use wired::{
    InvalidWiredConfigBlock, InvalidWiredMacroTable, WiredConfigBlock, WiredConfigReadError,
    WiredConfigWriteError, WiredKeymapWriteError, WiredMacroReadError, WiredMacroTable,
    WiredMacroWriteError, WiredModelProbe, WiredProbeError, discover_wired_f87_nodes,
    read_wired_config, read_wired_macro_table, read_wired_model_probe, write_wired_config,
    write_wired_keymap, write_wired_macro_table,
};

const FRAME_LENGTH: usize = 20;
const FRAGMENT_COUNT: usize = 10;
const READ_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(1);
const READ_TIMEOUT: Duration = Duration::from_secs(6);
const READ_ATTEMPTS: u8 = 5;
const READ_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x44, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x58,
];
const PSD_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x05, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x19,
];

/// Complete configuration, with frames in sequence order and checksums retained.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "backup::Backup", into = "backup::Backup")]
pub struct WirelessConfig {
    frames: [[u8; FRAME_LENGTH]; FRAGMENT_COUNT],
}

impl WirelessConfig {
    /// Ten verified input frames, ordered by their sequence number.
    #[must_use]
    pub fn frames(&self) -> &[[u8; FRAME_LENGTH]; FRAGMENT_COUNT] {
        &self.frames
    }

    /// Decode every currently supported F87 PRO setting from the snapshot.
    #[must_use]
    pub fn aula_state(&self) -> AulaState {
        AulaState::new(self.lighting(), SleepTimeout::from_wire(self.frames[1][15]))
            .with_debounce(AulaDebounce::from_wire(self.frames[0][8]))
            .with_side_light_effect(self.side_light_effect())
    }

    pub(super) fn side_light_effect(&self) -> Option<AulaSideLightEffect> {
        let (sequence, offset) = side_light_effect_location();
        AulaSideLightEffect::from_wire(self.frames[sequence][offset])
    }

    pub(super) fn set_side_light_effect(&mut self, effect: AulaSideLightEffect) {
        let (sequence, offset) = side_light_effect_location();
        self.frames[sequence][offset] = effect.wire_value();
    }
}

// Verified configuration byte 18 is payload byte 4 of fragment 1.
const fn side_light_effect_location() -> (usize, usize) {
    (1, 9)
}

/// Failure to obtain a complete, validated configuration.
#[derive(Debug, Error)]
pub enum ReadError {
    /// The requested device could not be selected uniquely.
    #[error("AULA device selection failed: {0}")]
    Selection(&'static str),
    /// Only the verified wireless control collection can be queried.
    #[error("not a verified AULA F87 PRO wireless control collection")]
    UnsupportedCollection,
    /// This OEM report-carrier diagnostic is not implemented on the current host.
    #[error("this AULA OEM report-carrier diagnostic is unavailable on the current host")]
    UnsupportedTransport,
    /// The backend could not open or communicate with the device.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// The complete transaction exceeded its deadline.
    #[error(
        "AULA keyboard did not return all requested reports; check that it is awake and paired to this receiver"
    )]
    Timeout,
    /// A matching configuration response had invalid framing or content.
    #[error("invalid AULA configuration response: {0}")]
    InvalidResponse(&'static str),
    /// The F87 Fn-layer response geometry differs from the expected matrix.
    #[error(
        "invalid AULA Fn matrix fragment: sequence {sequence}, response count {actual_fragments}, payload {actual_payload_length} bytes; expected {expected_fragments} fragments and {expected_payload_length} bytes at this sequence"
    )]
    InvalidFnMatrixFragment {
        /// Zero-based fragment sequence returned by the device.
        sequence: u8,
        /// Fragment count returned by the device.
        actual_fragments: u8,
        /// Payload length returned by the device.
        actual_payload_length: u8,
        /// Expected total fragment count.
        expected_fragments: usize,
        /// Expected payload length for this sequence.
        expected_payload_length: usize,
    },
}

/// Read configuration without changing settings.
///
/// The deadline covers opening, sending the single read request and receiving
/// all fragments. The session is dropped on every return, including cancellation.
/// Unrelated traffic is ignored within that same deadline; conflicting duplicate
/// fragments and malformed matching responses fail instead of publishing partial data.
pub(crate) async fn read_config(
    backend: &dyn HidBackend,
    node: &NodeInfo,
) -> Result<WirelessConfig, ReadError> {
    if !matches_control(
        node.vendor_id,
        node.product_id,
        node.usage_page,
        node.usage_id,
    ) {
        return Err(ReadError::UnsupportedCollection);
    }
    let mut phase = "opening the raw HID session";
    tokio::time::timeout(READ_TIMEOUT, async {
        tracing::debug!("AULA opening selected raw HID session");
        let mut session = backend.open_raw_session(node).await?;
        tracing::debug!("AULA raw HID session opened");
        phase = "sending the configuration request and receiving reports";
        read_session(&mut *session).await
    })
    .await
    .map_err(|_| {
        tracing::warn!(phase, "AULA configuration read exceeded its total deadline");
        ReadError::Timeout
    })?
}

async fn read_session(session: &mut dyn RawSession) -> Result<WirelessConfig, ReadError> {
    let frames = read_reports(session, &READ_REQUEST, config_frame)
        .await?
        .try_into()
        .map_err(|_| ReadError::InvalidResponse("wrong fragment count"))?;
    Ok(WirelessConfig { frames })
}

// All wireless reads share one bounded collector: no partial publication,
// conflicting duplicates, inconsistent counts or unrelated-traffic deadline reset.
async fn read_reports(
    session: &mut dyn RawSession,
    request: &[u8; FRAME_LENGTH],
    parse: impl Fn(&[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError>,
) -> Result<Vec<[u8; FRAME_LENGTH]>, ReadError> {
    read_reports_with_retries(session, request, parse).await
}

/// Retry one idempotent OEM read query up to five times after a response timeout.
///
/// Each attempt gets a fresh fragment collector but reuses the same request and
/// session. Invalid matching data and transport errors fail immediately.
pub(super) async fn read_reports_with_retries(
    session: &mut dyn RawSession,
    request: &[u8; FRAME_LENGTH],
    parse: impl Fn(&[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError>,
) -> Result<Vec<[u8; FRAME_LENGTH]>, ReadError> {
    for attempt in 0..READ_ATTEMPTS {
        match read_reports_with_timeout(session, request, &parse, READ_ATTEMPT_TIMEOUT).await {
            Ok(frames) => return Ok(frames),
            Err(ReadError::Timeout) if attempt + 1 < READ_ATTEMPTS => {
                tracing::debug!(
                    command = request[1],
                    attempt = attempt + 1,
                    "retrying timed-out AULA read request"
                );
            }
            Err(ReadError::Timeout) => {
                tracing::warn!(
                    command = request[1],
                    attempts = READ_ATTEMPTS,
                    "AULA read request exhausted retries"
                );
                return Err(ReadError::Timeout);
            }
            Err(error) => return Err(error),
        }
    }
    Err(ReadError::Timeout)
}

pub(super) async fn read_reports_with_timeout(
    session: &mut dyn RawSession,
    request: &[u8; FRAME_LENGTH],
    parse: impl Fn(&[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError>,
    timeout: Duration,
) -> Result<Vec<[u8; FRAME_LENGTH]>, ReadError> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut accepted_fragments = 0;
    let result = tokio::time::timeout_at(deadline, async {
        tracing::debug!(command = request[1], "AULA sending read request");
        let mut buffer = [0; 64];
        let mut pending_length = Some(session.exchange_output_report(request, &mut buffer).await?);
        tracing::debug!(
            command = request[1],
            "AULA read request sent; collecting fragments"
        );
        let mut fragments: Option<Vec<Option<[u8; FRAME_LENGTH]>>> = None;
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(ReadError::Timeout);
            }
            let length = match pending_length.take() {
                Some(length) => length,
                None => session.read_input_report(&mut buffer).await?,
            };
            tracing::debug!(command = request[1], length, "AULA input report received");
            if length == 0 {
                return Err(BackendError::Disconnected.into());
            }
            let report = buffer
                .get(..length)
                .ok_or(ReadError::InvalidResponse("oversized report"))?;
            let Some(frame) = parse(report)? else {
                continue;
            };
            let parts = fragments.get_or_insert_with(|| vec![None; usize::from(frame[2])]);
            if parts.len() != usize::from(frame[2]) {
                return Err(ReadError::InvalidResponse("inconsistent fragment count"));
            }
            let index = usize::from(frame[3]);
            if let Some(previous) = parts[index] {
                if previous != frame {
                    return Err(ReadError::InvalidResponse("conflicting duplicate fragment"));
                }
            } else {
                parts[index] = Some(frame);
                accepted_fragments += 1;
            }
            tracing::debug!(
                command = request[1],
                sequence = index,
                received = parts.iter().filter(|part| part.is_some()).count(),
                expected = parts.len(),
                "AULA read fragment accepted",
            );
            if parts.iter().all(Option::is_some) {
                return parts
                    .iter()
                    .copied()
                    .collect::<Option<Vec<_>>>()
                    .ok_or(ReadError::InvalidResponse("missing fragment"));
            }
        }
    })
    .await;
    if let Ok(result) = result {
        result
    } else {
        tracing::debug!(
            command = request[1],
            accepted_fragments,
            "AULA read attempt timed out"
        );
        Err(ReadError::Timeout)
    }
}

/// Select one verified keyboard, optionally by exact raw-route identity, and read it.
/// Ambiguous nodes are rejected before opening any device.
pub async fn read_selected_config(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WirelessConfig, ReadError> {
    let node = select_node(backend, identity).await?;
    read_config(backend, &node).await
}

/// Read the six-byte OEM model discriminator from one selected wireless F87 PRO.
///
/// This sends only the OEM's `0x13/0x05` identity query and does not alter settings.
pub async fn read_selected_psd(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<[u8; 6], ReadError> {
    let node = select_node(backend, identity).await?;
    tokio::time::timeout(READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(&node).await?;
        read_session_psd(&mut *session).await
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

pub(super) async fn read_session_psd(session: &mut dyn RawSession) -> Result<[u8; 6], ReadError> {
    let frames = read_reports_with_retries(session, &PSD_REQUEST, psd_frame).await?;
    let mut psd = Vec::with_capacity(6);
    for frame in frames {
        let length = usize::from(frame[4]);
        psd.extend_from_slice(&frame[5..5 + length]);
        if psd.len() >= 6 {
            break;
        }
    }
    psd.get(..6)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(ReadError::InvalidResponse("short OEM model discriminator"))
}

/// Resolve one verified control collection, rejecting ambiguous routes before I/O.
pub async fn select_node(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<NodeInfo, ReadError> {
    use crate::inventory::standalone::{standalone_device, validate_no_ambiguous_nodes};

    let nodes = backend.enumerate().await?;
    let candidates: Vec<_> = nodes
        .iter()
        .filter(|node| {
            matches_control(
                node.vendor_id,
                node.product_id,
                node.usage_page,
                node.usage_id,
            )
        })
        .collect();
    let devices: Vec<_> = candidates
        .iter()
        .filter_map(|node| standalone_device(node))
        .collect();
    validate_no_ambiguous_nodes(&devices)
        .map_err(|_| ReadError::Selection("ambiguous control collections"))?;
    let selected: Vec<_> = candidates
        .into_iter()
        .filter(|node| identity.is_none_or(|id| node.identity() == id))
        .collect();
    match selected.as_slice() {
        [node] => Ok((*node).clone()),
        [] => Err(ReadError::Selection("no matching wireless keyboard found")),
        _ => Err(ReadError::Selection(
            "multiple keyboards found; specify --identity",
        )),
    }
}

fn config_frame(report: &[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    let Some(frame) = validated_frame(report, 0x13, 0x44, FRAGMENT_COUNT)? else {
        return Ok(None);
    };
    if usize::from(frame[2]) != FRAGMENT_COUNT {
        return Err(ReadError::InvalidResponse(
            "wrong configuration fragment count",
        ));
    }
    let expected_length = if frame[3] == 9 { 2 } else { 14 };
    if frame[4] != expected_length {
        return Err(ReadError::InvalidResponse(
            "invalid fragment payload length",
        ));
    }
    if frame[3] == 9 && frame[5..7] != [0x5a, 0xa5] {
        return Err(ReadError::InvalidResponse(
            "missing configuration terminator",
        ));
    }
    Ok(Some(frame))
}

fn psd_frame(report: &[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    if !report.starts_with(&[0x13, 0x05]) {
        tracing::trace!(
            report_id = ?report.first(),
            command = ?report.get(1),
            length = report.len(),
            "ignoring unrelated report during OEM model query"
        );
        return Ok(None);
    }
    let Some(frame) = validated_frame(report, 0x13, 0x05, 6)? else {
        return Ok(None);
    };
    let length = usize::from(frame[4]);
    if !(1..=14).contains(&length) {
        return Err(ReadError::InvalidResponse(
            "invalid OEM discriminator fragment length",
        ));
    }
    Ok(Some(frame))
}

// Backups are canonical complete responses, not an arbitrary bag of valid frames.
fn validate_ordered_frames(
    frames: &[[u8; FRAME_LENGTH]],
    parse: impl Fn(&[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError>,
) -> Result<(), ReadError> {
    if frames.is_empty() {
        return Err(ReadError::InvalidResponse("empty response snapshot"));
    }
    for (sequence, frame) in frames.iter().enumerate() {
        if parse(frame)?.is_none()
            || usize::from(frame[3]) != sequence
            || usize::from(frame[2]) != frames.len()
        {
            return Err(ReadError::InvalidResponse(
                "snapshot must contain complete ordered READ reports",
            ));
        }
    }
    Ok(())
}

// Framing and allocation bounds are shared by every wireless read opcode.
fn validated_frame(
    report: &[u8],
    report_id: u8,
    command: u8,
    maximum: usize,
) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    if !report.starts_with(&[report_id, command]) {
        return Ok(None);
    }
    let frame: [u8; FRAME_LENGTH] = report
        .try_into()
        .map_err(|_| ReadError::InvalidResponse("wrong frame length"))?;
    if checksum(&frame) != frame[19] {
        return Err(ReadError::InvalidResponse("checksum mismatch"));
    }
    if frame[2] == 0 || usize::from(frame[2]) > maximum {
        return Err(ReadError::InvalidResponse("invalid fragment count"));
    }
    if frame[3] >= frame[2] {
        return Err(ReadError::InvalidResponse("invalid fragment sequence"));
    }
    Ok(Some(frame))
}

fn checksum(frame: &[u8; FRAME_LENGTH]) -> u8 {
    frame[..19]
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

#[cfg(test)]
mod tests;
