//! F87 PRO wireless keymap reads and writes.
//!
//! Entries remain raw four-byte values so unknown firmware actions survive a
//! read-modify-write. The OEM selects matrix layers separately from the matrix
//! command. The installed OEM uses report `0x15`; the same-model Linux
//! implementation uses the descriptor-advertised `0x13` wireless carrier.

use super::{
    FRAME_LENGTH, READ_TIMEOUT, ReadError, read_reports, select_node, validate_ordered_frames,
    validated_frame,
};
use crate::backend::HidBackend;
use openhid_core::hid::aula::{AulaBaseMatrix, AulaKeymap};
use openhid_device_registry::aula::matches_f87_pro_wireless_psd;

mod backup;

const MAXIMUM_BYTES: usize = 512; // The response header allows up to 128 entries.
const BASE_BYTES: usize = 126 * 4;
const FN_BYTES: usize = 126 * 4;
const MATRIX_FRAGMENT_BYTES: usize = 14;
const MATRIX_WRITE_REPORT_ID: u8 = 0x15;
const MATRIX_WRITE_COMMAND: u8 = 0x01;
const MATRIX_WRITE_INTER_FRAGMENT_DELAY: std::time::Duration = std::time::Duration::from_millis(15);
const MATRIX_WRITE_SETTLE_DELAY: std::time::Duration = std::time::Duration::from_millis(70);
const WIRELESS_MATRIX_WRITE_REPORT_ID: u8 = 0x13;
const MATRIX_WRITE_ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
const KEYMAP_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);
const MATRIX_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x55,
];
const OEM_MATRIX_REQUEST: [u8; FRAME_LENGTH] = [
    0x15, 0x41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x57,
];
const OEM_FN_MATRIX_REQUEST: [u8; FRAME_LENGTH] = [
    0x15, 0x41, 1, 0, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x67,
];
const FN_MATRIX_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x41, 1, 0, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x65,
];

/// Complete raw four-byte entries; unknown function encodings are retained.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "backup::Backup", into = "backup::Backup")]
pub struct WirelessMatrix {
    frames: Vec<[u8; FRAME_LENGTH]>,
    bytes: Vec<u8>,
}

/// OEM-shaped matrix write frames prepared without sending them to a device.
///
/// Static analysis of the installed F87 PRO driver establishes the frame
/// layout, but the receiver's acceptance of report ID `0x15` is unverified.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MatrixWritePlan {
    base: Vec<[u8; FRAME_LENGTH]>,
    fn_layer: Vec<[u8; FRAME_LENGTH]>,
}

impl MatrixWritePlan {
    /// Frames for the base layer, in OEM fragment order.
    #[must_use]
    pub fn base_frames(&self) -> &[[u8; FRAME_LENGTH]] {
        &self.base
    }

    /// Frames for the Fn layer, in OEM fragment order.
    #[must_use]
    pub fn fn_frames(&self) -> &[[u8; FRAME_LENGTH]] {
        &self.fn_layer
    }
}

/// Prepare static OEM-shaped frames for a complete keymap without device I/O.
///
/// The installed writer uses report `0x15`, command `0x01`, a layer selector in
/// the upper nibble of byte 4, at most 14 payload bytes at offsets 5–18, and an
/// additive checksum at byte 19. This only plans those frames; it does not
/// establish that the receiver accepts report `0x15`.
pub fn plan_keymap_write(target: &AulaKeymap) -> Result<MatrixWritePlan, KeymapPlanError> {
    Ok(MatrixWritePlan {
        base: matrix_write_frames(target.base(), 0)?,
        fn_layer: matrix_write_frames(target.fn_layer(), 1)?,
    })
}

/// A keymap cannot be represented by the OEM's one-byte fragment index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("AULA keymap exceeds the OEM fragment index range")]
pub struct KeymapPlanError;

/// Apply the OEM's observed response predicate for a wireless matrix write.
///
/// The OEM rejects report ID `0xff`, rejects byte-2 error bit `0x80`, and
/// matches byte 1 to command `0x01`; it does not check a reply checksum.
#[must_use]
pub fn is_matrix_write_ack(report: &[u8]) -> bool {
    report.len() == FRAME_LENGTH
        && report[0] != 0xff
        && report[1] == MATRIX_WRITE_COMMAND
        && report[2] & 0x80 == 0
}

fn matrix_write_frames(
    entries: &[[u8; 4]],
    layer: u8,
) -> Result<Vec<[u8; FRAME_LENGTH]>, KeymapPlanError> {
    let bytes: Vec<_> = entries.iter().flatten().copied().collect();
    bytes
        .chunks(MATRIX_FRAGMENT_BYTES)
        .enumerate()
        .map(|(index, payload)| {
            let mut frame = [0; FRAME_LENGTH];
            frame[0] = MATRIX_WRITE_REPORT_ID;
            frame[1] = MATRIX_WRITE_COMMAND;
            frame[2] = 1;
            frame[3] = u8::try_from(index).map_err(|_| KeymapPlanError)?;
            frame[4] = (layer << 4) | u8::try_from(payload.len()).map_err(|_| KeymapPlanError)?;
            frame[5..5 + payload.len()].copy_from_slice(payload);
            frame[19] = super::checksum(&frame);
            Ok(frame)
        })
        .collect()
}

impl WirelessMatrix {
    /// Complete canonical response frames, including every original payload byte.
    #[must_use]
    pub fn frames(&self) -> &[[u8; FRAME_LENGTH]] {
        &self.frames
    }

    /// Validate a complete ordered response before accepting a matrix backup.
    /// Function encodings remain opaque; no missing slots are filled from defaults.
    pub fn from_frames(frames: Vec<[u8; FRAME_LENGTH]>) -> Result<Self, ReadError> {
        super::validate_ordered_frames(&frames, matrix_frame)?;
        let bytes: Vec<_> = frames
            .iter()
            .flat_map(|frame| frame[5..5 + usize::from(frame[4])].iter().copied())
            .collect();
        if bytes.len() > MAXIMUM_BYTES || !bytes.len().is_multiple_of(4) {
            return Err(ReadError::InvalidResponse("invalid key matrix size"));
        }
        Ok(Self { frames, bytes })
    }

    /// Device-provided entries in their original order. No key names are assumed.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        self.bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(<[u8; 4]>::as_slice)
    }

    /// Validate and retain the 126-entry F87 PRO base layer as a shared model.
    pub fn aula_base_matrix(&self) -> Result<AulaBaseMatrix, ReadError> {
        let entries = self.bytes.as_chunks::<4>().0.to_vec();
        AulaBaseMatrix::new(entries)
            .map_err(|_| ReadError::InvalidResponse("invalid base matrix entry count"))
    }
}

/// Experimental matrix read on one uniquely selected wireless control collection.
/// Sends only a read request, never a key assignment, SAVE or reset command.
/// A complete response must fit the OEM matrix bound and contain whole entries.
pub async fn read_selected_matrix(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WirelessMatrix, ReadError> {
    read_selected_matrix_with_request(backend, identity, &MATRIX_REQUEST).await
}

/// Read the base matrix with the OEM's `0x15` request carrier.
///
/// This is a read-only transport diagnostic for Linux hidraw and Windows raw HID,
/// which may pass report IDs absent from the collection descriptor. It
/// does not establish that the receiver accepts the request, and never sends a
/// matrix assignment, SAVE, or reset command.
pub async fn read_selected_matrix_with_oem_carrier(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WirelessMatrix, ReadError> {
    read_selected_matrix_with_request(backend, identity, &OEM_MATRIX_REQUEST).await
}

/// Read and validate both keymap layers through the OEM's unadvertised `0x15` carrier.
///
/// This diagnostic is read-only and intended for raw HID transport paths.
/// It sends OEM `0x15` requests and validates the expected `0x13` input reports.
/// The Fn-layer request and report acceptance remain unverified on hardware.
pub async fn read_selected_keymap_with_oem_carrier(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<AulaKeymap, ReadError> {
    let node = select_node(backend, identity).await?;
    tokio::time::timeout(KEYMAP_READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(&node).await?;
        let base_frames = read_reports(&mut *session, &OEM_MATRIX_REQUEST, |report| {
            matrix_frame_with_report_id(report, 0x13)
        })
        .await?;
        let fn_frames = read_reports(&mut *session, &OEM_FN_MATRIX_REQUEST, |report| {
            fn_matrix_frame_with_report_id(report, 0x13)
        })
        .await?;
        let base_bytes = matrix_bytes(&base_frames, 0x13, 0x41, BASE_BYTES)?;
        let fn_bytes = matrix_bytes(&fn_frames, 0x13, 0x41, FN_BYTES)?;
        AulaKeymap::new(
            base_bytes.as_chunks::<4>().0.to_vec(),
            fn_bytes.as_chunks::<4>().0.to_vec(),
        )
        .map_err(|_| ReadError::InvalidResponse("invalid keymap entry count"))
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

async fn read_selected_matrix_with_request(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    request: &[u8; FRAME_LENGTH],
) -> Result<WirelessMatrix, ReadError> {
    let node = select_node(backend, identity).await?;
    tokio::time::timeout(READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(&node).await?;
        let frames = read_reports(&mut *session, request, matrix_frame).await?;
        WirelessMatrix::from_frames(frames)
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

/// Read the verified 126-entry base layer for use by IPC clients and editors.
pub async fn read_selected_base_matrix(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<AulaBaseMatrix, ReadError> {
    read_selected_matrix(backend, identity)
        .await?
        .aula_base_matrix()
}

/// Attempt to read both wireless keymap layers without changing device state.
///
/// The Fn-layer selector follows the installed OEM method. Both layer response
/// geometry and the selector byte have been confirmed on an F87 PRO receiver.
pub async fn read_selected_keymap(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<AulaKeymap, ReadError> {
    let node = select_node(backend, identity).await?;
    tokio::time::timeout(KEYMAP_READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(&node).await?;
        read_session_keymap(&mut *session).await
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

/// Write and verify both F87 Pro wireless keymap layers using the advertised
/// `0x13` carrier used by the independent same-model Linux implementation.
///
/// The installed Windows OEM driver uses a separate, unadvertised `0x15`
/// carrier. This path sends one acknowledged fragment at a time over the
/// receiver's advertised 20-byte `0x13` output report, then reads both layers
/// back. A failed or cancelled update attempts to restore and verify the
/// complete original keymap before returning.
pub async fn update_selected_keymap(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    target: AulaKeymap,
    cancelled: impl Fn() -> bool,
) -> Result<AulaKeymap, KeymapWriteError> {
    update_selected_keymap_inner(backend, identity, None, target, cancelled).await
}

/// Write and verify a complete wireless keymap only if it still matches the
/// snapshot the caller edited.
pub async fn update_selected_keymap_if_unchanged(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    expected: AulaKeymap,
    target: AulaKeymap,
    cancelled: impl Fn() -> bool,
) -> Result<AulaKeymap, KeymapWriteError> {
    update_selected_keymap_inner(backend, identity, Some(expected), target, cancelled).await
}

async fn update_selected_keymap_inner(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    expected: Option<AulaKeymap>,
    target: AulaKeymap,
    cancelled: impl Fn() -> bool,
) -> Result<AulaKeymap, KeymapWriteError> {
    let node = select_node(backend, identity).await?;
    let mut session = backend
        .open_raw_session(&node)
        .await
        .map_err(ReadError::from)?;
    let psd = super::read_session_psd(&mut *session).await?;
    if !matches_f87_pro_wireless_psd(&psd) {
        return Err(KeymapWriteError::UnsupportedModel(psd));
    }
    let before = read_session_keymap(&mut *session).await?;
    if expected
        .as_ref()
        .is_some_and(|expected| expected != &before)
    {
        return Err(KeymapWriteError::StaleSnapshot);
    }
    if before == target {
        return Ok(before);
    }
    if cancelled() {
        return Err(KeymapWriteError::Cancelled);
    }

    let cause = match write_keymap(&mut *session, &target, &cancelled).await {
        Ok(()) => match read_session_keymap(&mut *session).await {
            Ok(actual) if actual == target => return Ok(actual),
            Ok(_) => KeymapWriteError::ReadbackMismatch,
            Err(error) => error.into(),
        },
        Err(error) => error,
    };

    if let Err(recovery) = write_keymap(&mut *session, &before, &|| false).await {
        return Err(KeymapWriteError::RecoveryFailed {
            cause: Box::new(cause),
            recovery: Box::new(recovery),
        });
    }
    match read_session_keymap(&mut *session).await {
        Ok(actual) if actual == before => Err(KeymapWriteError::Restored(Box::new(cause))),
        Ok(_) => Err(KeymapWriteError::RecoveryFailed {
            cause: Box::new(cause),
            recovery: Box::new(KeymapWriteError::ReadbackMismatch),
        }),
        Err(error) => Err(KeymapWriteError::RecoveryFailed {
            cause: Box::new(cause),
            recovery: Box::new(error.into()),
        }),
    }
}

/// Failure while applying or recovering a wireless F87 Pro keymap.
#[derive(Debug, thiserror::Error)]
pub enum KeymapWriteError {
    /// Device query, readback, or raw transport failed.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// The live six-byte model discriminator is not the F87 Pro profile.
    #[error("wireless AULA device has unsupported PSD {0:02x?}; expected F87 Pro")]
    UnsupportedModel([u8; 6]),
    /// Caller requested cancellation before or during the update.
    #[error("AULA keymap update cancelled")]
    Cancelled,
    /// The keyboard changed after the caller loaded the keymap for editing.
    #[error("AULA keymap changed since it was loaded; refresh it before applying")]
    StaleSnapshot,
    /// A matching fragment acknowledgement did not arrive before its deadline.
    #[error("AULA keymap write acknowledgement timed out for layer {layer}, fragment {sequence}")]
    AckTimeout {
        /// Matrix layer, where 0 is base and 1 is Fn.
        layer: u8,
        /// Zero-based fragment number.
        sequence: u8,
    },
    /// A matching fragment acknowledgement was malformed or rejected.
    #[error("AULA keymap write rejected for layer {layer}, fragment {sequence}")]
    WriteRejected {
        /// Matrix layer, where 0 is base and 1 is Fn.
        layer: u8,
        /// Zero-based fragment number.
        sequence: u8,
    },
    /// Readback did not match the requested keymap.
    #[error("AULA keymap readback does not match the requested mapping")]
    ReadbackMismatch,
    /// The attempted update failed but the original keymap was restored and verified.
    #[error("AULA keymap update failed; the original mapping was restored: {0}")]
    Restored(Box<Self>),
    /// Both the attempted update and restoration failed verification.
    #[error("AULA keymap update failed ({cause}); restoration failed ({recovery})")]
    RecoveryFailed {
        /// Failure that triggered restoration.
        cause: Box<Self>,
        /// Failure while restoring or verifying the original mapping.
        recovery: Box<Self>,
    },
    /// A fixed-size F87 Pro mapping exceeded the packet sequence range.
    #[error(transparent)]
    Plan(#[from] KeymapPlanError),
}

async fn write_keymap(
    session: &mut dyn crate::backend::RawSession,
    keymap: &AulaKeymap,
    cancelled: &impl Fn() -> bool,
) -> Result<(), KeymapWriteError> {
    for (layer, entries) in [(0, keymap.base()), (1, keymap.fn_layer())] {
        for frame in wireless_matrix_write_frames(entries, layer)? {
            if cancelled() {
                return Err(KeymapWriteError::Cancelled);
            }
            tokio::time::sleep(MATRIX_WRITE_INTER_FRAGMENT_DELAY).await;
            await_matrix_write_ack(session, &frame, layer).await?;
        }
    }
    tokio::time::sleep(MATRIX_WRITE_SETTLE_DELAY).await;
    Ok(())
}

fn wireless_matrix_write_frames(
    entries: &[[u8; 4]],
    layer: u8,
) -> Result<Vec<[u8; FRAME_LENGTH]>, KeymapWriteError> {
    let bytes: Vec<_> = entries.iter().flatten().copied().collect();
    let fragment_count = bytes.len().div_ceil(MATRIX_FRAGMENT_BYTES);
    let total = u8::try_from(fragment_count).map_err(|_| KeymapPlanError)?;
    bytes
        .chunks(MATRIX_FRAGMENT_BYTES)
        .enumerate()
        .map(|(index, payload)| {
            let mut frame = [0; FRAME_LENGTH];
            frame[0] = WIRELESS_MATRIX_WRITE_REPORT_ID;
            frame[1] = MATRIX_WRITE_COMMAND;
            frame[2] = total;
            frame[3] = u8::try_from(index).map_err(|_| KeymapPlanError)?;
            frame[4] = (layer << 4) | u8::try_from(payload.len()).map_err(|_| KeymapPlanError)?;
            frame[5..5 + payload.len()].copy_from_slice(payload);
            frame[19] = super::checksum(&frame);
            Ok(frame)
        })
        .collect()
}

async fn await_matrix_write_ack(
    session: &mut dyn crate::backend::RawSession,
    request: &[u8; FRAME_LENGTH],
    layer: u8,
) -> Result<(), KeymapWriteError> {
    let deadline = tokio::time::Instant::now() + MATRIX_WRITE_ACK_TIMEOUT;
    let mut buffer = [0; 64];
    let mut pending_length = Some(
        tokio::time::timeout_at(
            deadline,
            session.exchange_output_report(request, &mut buffer),
        )
        .await
        .map_err(|_| KeymapWriteError::AckTimeout {
            layer,
            sequence: request[3],
        })?
        .map_err(ReadError::from)?,
    );
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(KeymapWriteError::AckTimeout {
                layer,
                sequence: request[3],
            });
        }
        let length = match pending_length.take() {
            Some(length) => length,
            None => tokio::time::timeout_at(deadline, session.read_input_report(&mut buffer))
                .await
                .map_err(|_| KeymapWriteError::AckTimeout {
                    layer,
                    sequence: request[3],
                })?
                .map_err(ReadError::from)?,
        };
        if length == 0 {
            return Err(ReadError::Backend(crate::backend::BackendError::Disconnected).into());
        }
        let Some(report) = buffer.get(..length) else {
            return Err(
                ReadError::InvalidResponse("oversized matrix write acknowledgement").into(),
            );
        };
        if report.first() != Some(&WIRELESS_MATRIX_WRITE_REPORT_ID)
            || report.get(1) != Some(&MATRIX_WRITE_COMMAND)
        {
            continue;
        }
        let Ok(frame) = <&[u8; FRAME_LENGTH]>::try_from(report) else {
            return Err(KeymapWriteError::WriteRejected {
                layer,
                sequence: request[3],
            });
        };
        if super::checksum(frame) != frame[19] {
            return Err(ReadError::InvalidResponse(
                "matrix write acknowledgement checksum mismatch",
            )
            .into());
        }
        if frame[2] & 0x80 != 0
            || frame[2] == 0
            || frame[2] > 127
            || frame[3] >= frame[2]
            || frame[4] & 15 > 14
        {
            return Err(KeymapWriteError::WriteRejected {
                layer,
                sequence: request[3],
            });
        }
        if frame[3] == request[3] && frame[4] >> 4 == layer {
            return Ok(());
        }
    }
}

async fn read_session_keymap(
    session: &mut dyn crate::backend::RawSession,
) -> Result<AulaKeymap, ReadError> {
    let base_frames = read_reports(session, &MATRIX_REQUEST, matrix_frame).await?;
    let fn_frames = read_reports(session, &FN_MATRIX_REQUEST, fn_matrix_frame).await?;
    let base_bytes = matrix_bytes(&base_frames, 0x13, 0x41, BASE_BYTES)?;
    let fn_bytes = matrix_bytes(&fn_frames, 0x13, 0x41, FN_BYTES)?;
    let base = base_bytes.as_chunks::<4>().0.to_vec();
    let fn_layer = fn_bytes.as_chunks::<4>().0.to_vec();
    AulaKeymap::new(base, fn_layer)
        .map_err(|_| ReadError::InvalidResponse("invalid keymap entry count"))
}

fn matrix_frame(report: &[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    matrix_frame_with_report_id(report, 0x13)
}

fn matrix_frame_with_report_id(
    report: &[u8],
    report_id: u8,
) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    let Some(frame) = validated_frame(
        report,
        report_id,
        0x41,
        MAXIMUM_BYTES.div_ceil(MATRIX_FRAGMENT_BYTES),
    )?
    else {
        return Ok(None);
    };
    if frame[4] >> 4 != 0 {
        return Err(ReadError::InvalidResponse(
            "unexpected matrix layer selector",
        ));
    }
    let length = usize::from(frame[4] & 0x0f);
    if !(1..=MATRIX_FRAGMENT_BYTES).contains(&length)
        || (frame[3] + 1 != frame[2] && length != MATRIX_FRAGMENT_BYTES)
    {
        return Err(ReadError::InvalidResponse(
            "invalid matrix fragment payload length",
        ));
    }
    Ok(Some(frame))
}

fn fn_matrix_frame(report: &[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    fn_matrix_frame_with_report_id(report, 0x13)
}

fn fn_matrix_frame_with_report_id(
    report: &[u8],
    report_id: u8,
) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    let Some(frame) = validated_frame(
        report,
        report_id,
        0x41,
        FN_BYTES.div_ceil(MATRIX_FRAGMENT_BYTES),
    )?
    else {
        return Ok(None);
    };
    let expected_pages = FN_BYTES.div_ceil(MATRIX_FRAGMENT_BYTES);
    let expected_last_length = last_fragment_length(FN_BYTES);
    let expected_length = if usize::from(frame[3]) + 1 == expected_pages {
        expected_last_length
    } else {
        MATRIX_FRAGMENT_BYTES
    };
    let actual_layer = frame[4] >> 4;
    let actual_length = frame[4] & 0x0f;
    if actual_layer != 1
        || usize::from(frame[2]) != expected_pages
        || usize::from(actual_length) != expected_length
    {
        return Err(ReadError::InvalidFnMatrixFragment {
            sequence: frame[3],
            actual_fragments: frame[2],
            actual_payload_length: actual_length,
            expected_fragments: expected_pages,
            expected_payload_length: expected_length,
        });
    }
    Ok(Some(frame))
}

fn matrix_bytes(
    frames: &[[u8; FRAME_LENGTH]],
    report_id: u8,
    command: u8,
    expected_length: usize,
) -> Result<Vec<u8>, ReadError> {
    validate_ordered_frames(frames, |report| {
        let Some(frame) = validated_frame(
            report,
            report_id,
            command,
            expected_length.div_ceil(MATRIX_FRAGMENT_BYTES),
        )?
        else {
            return Ok(None);
        };
        let expected_pages = expected_length.div_ceil(MATRIX_FRAGMENT_BYTES);
        let expected_last_length = last_fragment_length(expected_length);
        let expected_fragment_length = if usize::from(frame[3]) + 1 == expected_pages {
            expected_last_length
        } else {
            MATRIX_FRAGMENT_BYTES
        };
        if usize::from(frame[2]) != expected_pages
            || usize::from(frame[4] & 0x0f) != expected_fragment_length
        {
            return Err(ReadError::InvalidResponse("invalid keymap fragment size"));
        }
        Ok(Some(frame))
    })?;
    let bytes: Vec<_> = frames
        .iter()
        .flat_map(|frame| frame[5..5 + usize::from(frame[4] & 0x0f)].iter().copied())
        .collect();
    if bytes.len() != expected_length {
        return Err(ReadError::InvalidResponse("incomplete keymap matrix"));
    }
    Ok(bytes)
}

fn last_fragment_length(total_length: usize) -> usize {
    match total_length % MATRIX_FRAGMENT_BYTES {
        0 => MATRIX_FRAGMENT_BYTES,
        remainder => remainder,
    }
}
