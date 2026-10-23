//! Experimental read-only query for a prefix of F87 PRO wireless macro memory.

use std::time::Duration;

use super::{FRAME_LENGTH, ReadError, read_reports_with_timeout, select_node, validated_frame};
use crate::backend::{HidBackend, RawSession};

const MACRO_READ_ROUND_LENGTH: usize = 512;
const GENERIC_OEM_AGGREGATE_LIMIT: usize = 0x2800;
const MACRO_READ_ROUND_COUNT: usize = GENERIC_OEM_AGGREGATE_LIMIT / MACRO_READ_ROUND_LENGTH;
const MACRO_FRAGMENT_LENGTH: usize = 14;
const MACRO_FRAGMENT_COUNT: usize = MACRO_READ_ROUND_LENGTH.div_ceil(MACRO_FRAGMENT_LENGTH);
const MACRO_READ_TIMEOUT: Duration = Duration::from_secs(12);
// Twenty serialized rounds can each consume the full per-round deadline.
// Leave one minute beyond that worst-case budget for opening and scheduling.
const MACRO_TABLE_READ_TIMEOUT: Duration = Duration::from_secs(300);
const MACRO_READ_RESPONSE_ID: u8 = 0x13;
const MACRO_READ_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x43, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x57,
];

/// The 512-byte prefix returned by the experimental F87 PRO wireless macro query.
///
/// This response length does not prove the F87 macro table's size or
/// completeness. Do not reconstruct a full table from this prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WirelessMacroPrefix {
    frames: Vec<[u8; FRAME_LENGTH]>,
    bytes: Vec<u8>,
}

impl WirelessMacroPrefix {
    /// Validated query response frames in sequence order.
    #[must_use]
    pub fn frames(&self) -> &[[u8; FRAME_LENGTH]] {
        &self.frames
    }

    /// Raw bytes returned by the query. This is only a prefix of macro memory.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Candidate raw data from repeated OEM-shaped wireless macro reads.
///
/// The generic OEM aggregate encoder caps its transfer table at 0x2800 bytes;
/// that cap is not established as the F87's capacity. Whether this receiver's
/// repeated `0x43` responses advance through a table is also unverified, so
/// this value is diagnostic data, not a restorable macro backup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WirelessMacroTableCandidate {
    reads: Vec<Vec<u8>>,
    bytes: Vec<u8>,
}

impl WirelessMacroTableCandidate {
    /// Individual 512-byte responses in request order. This does not prove
    /// that the device advanced to another address between repeated queries.
    #[must_use]
    pub fn reads(&self) -> &[Vec<u8>] {
        &self.reads
    }

    /// Raw bytes returned by all candidate read rounds.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Read the experimental 512-byte macro-memory prefix over the F87 PRO receiver.
///
/// This uses the advertised wireless `0x13` report ID and the `GetMacro` `0x43`
/// command described by the same-model Linux implementation. The installed
/// Windows OEM instead sends this query over unadvertised report ID `0x15`;
/// Windows rejects that output report on the current receiver collection.
/// This reads no settings, and the returned prefix is not a complete macro
/// backup or sufficient input for editing existing macros.
pub async fn read_selected_macro_prefix(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WirelessMacroPrefix, ReadError> {
    let node = select_node(backend, identity).await?;
    tokio::time::timeout(MACRO_READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(&node).await?;
        read_macro_response_round(&mut *session).await
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

/// Read a candidate sized to the generic OEM aggregate encoder's 0x2800-byte
/// limit through the wireless `GetMacro` command, repeating the same request
/// on one serialized session.
///
/// The installed OEM `GetMacro` reader accepts a caller-provided buffer length
/// and repeats its request until that length is filled. The 0x2800-byte bound
/// comes from a generic aggregate path whose destination class is not the F87
/// wireless `CDevComboFilm`; it is only a candidate bound here. This mirrors the
/// reader loop using twenty 512-byte reads. The F87 receiver has only returned
/// one response round so far; address advancement and completeness remain
/// unverified. No data is written to the device.
pub async fn read_selected_macro_table_candidate(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WirelessMacroTableCandidate, ReadError> {
    let node = select_node(backend, identity).await?;
    tokio::time::timeout(MACRO_TABLE_READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(&node).await?;
        let mut reads = Vec::with_capacity(MACRO_READ_ROUND_COUNT);
        let mut bytes = Vec::with_capacity(GENERIC_OEM_AGGREGATE_LIMIT);
        for _ in 0..MACRO_READ_ROUND_COUNT {
            let response = read_macro_response_round(&mut *session).await?;
            bytes.extend_from_slice(response.bytes());
            reads.push(response.bytes().to_vec());
        }
        Ok(WirelessMacroTableCandidate { reads, bytes })
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

async fn read_macro_response_round(
    session: &mut dyn RawSession,
) -> Result<WirelessMacroPrefix, ReadError> {
    let frames = read_reports_with_timeout(
        session,
        &MACRO_READ_REQUEST,
        macro_prefix_frame,
        MACRO_READ_TIMEOUT,
    )
    .await?;
    let bytes = macro_prefix_bytes(&frames)?;
    Ok(WirelessMacroPrefix { frames, bytes })
}

fn macro_prefix_frame(report: &[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    let Some(frame) = validated_frame(
        report,
        MACRO_READ_RESPONSE_ID,
        MACRO_READ_REQUEST[1],
        MACRO_FRAGMENT_COUNT,
    )?
    else {
        return Ok(None);
    };
    if frame[4] >> 4 != 0 {
        return Ok(None);
    }
    if usize::from(frame[2]) != MACRO_FRAGMENT_COUNT {
        return Err(ReadError::InvalidResponse(
            "wrong wireless macro-prefix fragment count",
        ));
    }
    let expected_length = if usize::from(frame[3]) + 1 == MACRO_FRAGMENT_COUNT {
        MACRO_READ_ROUND_LENGTH % MACRO_FRAGMENT_LENGTH
    } else {
        MACRO_FRAGMENT_LENGTH
    };
    if usize::from(frame[4] & 0x0f) != expected_length {
        return Err(ReadError::InvalidResponse(
            "invalid wireless macro-prefix fragment size",
        ));
    }
    Ok(Some(frame))
}

fn macro_prefix_bytes(frames: &[[u8; FRAME_LENGTH]]) -> Result<Vec<u8>, ReadError> {
    if frames.len() != MACRO_FRAGMENT_COUNT {
        return Err(ReadError::InvalidResponse(
            "incomplete wireless macro-prefix response",
        ));
    }
    frames.iter().try_fold(
        Vec::with_capacity(MACRO_READ_ROUND_LENGTH),
        |mut bytes, frame| {
            let parsed = macro_prefix_frame(frame)?.ok_or(ReadError::InvalidResponse(
                "invalid wireless macro-prefix frame",
            ))?;
            let length = usize::from(parsed[4] & 0x0f);
            bytes.extend_from_slice(&parsed[5..5 + length]);
            Ok(bytes)
        },
    )
}
