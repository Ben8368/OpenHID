//! Experimental read-only wireless key matrix. No assignments are written.
//!
//! Opcode 0x41 is documented by the reference study; payload meaning is kept
//! opaque until independently checked against this physical F87 PRO.

use super::{FRAME_LENGTH, READ_TIMEOUT, ReadError, read_reports, select_node, validated_frame};
use crate::backend::HidBackend;

mod backup;

const MAXIMUM_BYTES: usize = 512; // OEM MatrixLen=128, four bytes per entry.
const MATRIX_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x55,
];

/// Complete raw four-byte entries; unknown function encodings are retained.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "backup::Backup", into = "backup::Backup")]
pub struct WirelessMatrix {
    frames: Vec<[u8; FRAME_LENGTH]>,
    bytes: Vec<u8>,
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
}

/// Experimental matrix read on one uniquely selected wireless control collection.
/// Sends only a read request, never a key assignment, SAVE or reset command.
/// A complete response must fit the OEM matrix bound and contain whole entries.
pub async fn read_selected_matrix(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WirelessMatrix, ReadError> {
    let node = select_node(backend, identity).await?;
    tokio::time::timeout(READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(&node).await?;
        let frames = read_reports(&mut *session, &MATRIX_REQUEST, matrix_frame).await?;
        WirelessMatrix::from_frames(frames)
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

fn matrix_frame(report: &[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    let Some(frame) = validated_frame(report, 0x41, MAXIMUM_BYTES.div_ceil(14))? else {
        return Ok(None);
    };
    let length = usize::from(frame[4]);
    if !(1..=14).contains(&length) || (frame[3] + 1 != frame[2] && length != 14) {
        return Err(ReadError::InvalidResponse(
            "invalid matrix fragment payload length",
        ));
    }
    Ok(Some(frame))
}
