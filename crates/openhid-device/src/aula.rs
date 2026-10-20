//! F87 PRO wireless configuration protocol.
//!
//! Frames were verified on a 3554:FA09 receiver. Unknown configuration bytes
//! are preserved. Only independently verified controls are exposed.

use std::time::Duration;

use openhid_device_registry::aula::matches_control;
use thiserror::Error;

use crate::backend::{BackendError, HidBackend, NodeInfo, RawSession};

mod backup;
mod lighting;
mod matrix;
mod transaction;
pub use lighting::{LightingChange, LightingMode, LightingState, update_lighting};
pub use matrix::{WirelessMatrix, read_selected_matrix};
pub use openhid_core::hid::aula::{AulaChange, AulaState, SleepTimeout};
pub use transaction::{ConfigChange, ConfigError, ConfigResult, update_config};

const FRAME_LENGTH: usize = 20;
const FRAGMENT_COUNT: usize = 10;
const READ_TIMEOUT: Duration = Duration::from_secs(2);
const READ_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x44, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x58,
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
    }
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
    /// The backend could not open or communicate with the device.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// The complete transaction exceeded its deadline.
    #[error("AULA read timed out before all requested fragments arrived")]
    Timeout,
    /// A matching configuration response had invalid framing or content.
    #[error("invalid AULA configuration response: {0}")]
    InvalidResponse(&'static str),
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
    tokio::time::timeout(READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(node).await?;
        read_session(&mut *session).await
    })
    .await
    .map_err(|_| ReadError::Timeout)?
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
    let deadline = tokio::time::Instant::now() + READ_TIMEOUT;
    tokio::time::timeout_at(deadline, async {
        session.write_output_report(request).await?;
        let mut fragments: Option<Vec<Option<[u8; FRAME_LENGTH]>>> = None;
        let mut buffer = [0; 64];
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(ReadError::Timeout);
            }
            let length = session.read_input_report(&mut buffer).await?;
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
            }
            if parts.iter().all(Option::is_some) {
                return parts
                    .iter()
                    .copied()
                    .collect::<Option<Vec<_>>>()
                    .ok_or(ReadError::InvalidResponse("missing fragment"));
            }
        }
    })
    .await
    .map_err(|_| ReadError::Timeout)?
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
    let Some(frame) = validated_frame(report, 0x44, FRAGMENT_COUNT)? else {
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
    command: u8,
    maximum: usize,
) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    if !report.starts_with(&[0x13, command]) {
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
