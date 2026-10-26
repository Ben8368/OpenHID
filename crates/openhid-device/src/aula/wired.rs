//! Model discovery and keymap upload over the wired F87 Pro Feature Report protocol.

use std::time::Duration;

use thiserror::Error;

use crate::backend::{BackendError, HidBackend, NodeInfo};
use openhid_core::hid::aula::{
    AULA_WIRED_CONFIG_BLOCK_BYTE_LENGTH, AULA_WIRED_CONFIG_BLOCK_LENGTH, AulaWiredKeymapBlob,
    InvalidAulaWiredKeymapBlob,
};
pub use openhid_core::hid::aula::{
    AulaWiredConfigBlock as WiredConfigBlock,
    InvalidAulaWiredConfigBlock as InvalidWiredConfigBlock,
};

const FEATURE_REPORT_ID: u8 = 0x06;
const MODEL_QUERY_COMMAND: u8 = 0x82;
const MODEL_FAMILY_CODE: u8 = 0x03;
const MODEL_QUERY_RESPONSE_LENGTH: usize = 14;
const FEATURE_REPORT_LENGTH: usize = 520;
const MODEL_QUERY_TIMEOUT: Duration = Duration::from_secs(8);
const F87_PRO_MODEL_CODE: u8 = 0x0b;
const MODEL_CODE_OFFSET: usize = 13;
const CONFIG_READ_COMMAND: u8 = 0x84;
const CONFIG_WRITE_COMMAND: u8 = 0x04;
const MACRO_READ_COMMAND: u8 = 0x85;
const MACRO_READ_CHUNKS: u8 = 8;
const MACRO_CHUNK_LENGTH: usize = 512;
const MACRO_TABLE_LENGTH: usize = MACRO_CHUNK_LENGTH * MACRO_READ_CHUNKS as usize;
const CONFIG_HEADER_LENGTH: usize = 8;
const CONFIG_TRANSFER_LENGTH: usize = CONFIG_HEADER_LENGTH + AULA_WIRED_CONFIG_BLOCK_BYTE_LENGTH;
const CONFIG_WRITE_SETTLE: Duration = Duration::from_millis(350);
const MACRO_READ_TIMEOUT: Duration = Duration::from_secs(20);

/// Failure to read one complete validated wired F87 Pro configuration block.
#[derive(Debug, Error)]
pub enum WiredConfigReadError {
    /// Reads require the exact identity printed by the read-only wired model probe.
    #[error(
        "wired AULA configuration reads require the exact identity from `openhid diag aula --wired-model-probe`"
    )]
    IdentityRequired,
    /// No matching direct USB keyboard collection was found.
    #[error("no AULA wired keyboard collection (258A:010C) was found")]
    NotFound,
    /// The supplied identity did not select exactly one matching collection.
    #[error("AULA wired collection selection failed: {0}")]
    Selection(&'static str),
    /// The selected collection did not pass the F87 Pro model query.
    #[error("AULA wired configuration read refused because the model query failed: {0}")]
    ModelProbe(#[source] Box<WiredProbeError>),
    /// The selected HID Feature Report could not be opened or read.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// The device returned a response that does not match the configuration query.
    #[error("invalid AULA wired configuration response: {0}")]
    InvalidResponse(&'static str),
    /// The device returned a block without known healthy markers.
    #[error(transparent)]
    InvalidBlock(#[from] InvalidWiredConfigBlock),
}

/// Failure to update a wired F87 Pro config block, including recovery status.
#[derive(Debug, Error)]
pub enum WiredConfigWriteError {
    /// Writes require the exact identity printed by the read-only wired model probe.
    #[error(
        "wired AULA configuration writes require the exact identity from `openhid diag aula --wired-model-probe`"
    )]
    IdentityRequired,
    /// No matching direct USB keyboard collection was found.
    #[error("no AULA wired keyboard collection (258A:010C) was found")]
    NotFound,
    /// The supplied identity did not select exactly one matching collection.
    #[error("AULA wired collection selection failed: {0}")]
    Selection(&'static str),
    /// The target block does not pass healthy-marker validation.
    #[error(transparent)]
    InvalidBlock(#[from] InvalidWiredConfigBlock),
    /// The selected collection did not pass the F87 Pro model query.
    #[error("AULA wired configuration write refused because the model query failed: {0}")]
    ModelProbe(#[source] Box<WiredProbeError>),
    /// The selected HID Feature Report could not be opened or written.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// The current settings could not be read before writing.
    #[error("could not read the original AULA wired configuration: {0}")]
    ReadOriginal(#[source] Box<WiredConfigReadError>),
    /// The write failed; `recovery` reports whether the original block was restored.
    #[error("AULA wired configuration write failed: {cause}; original-block recovery: {recovery}")]
    WriteRecovery {
        /// Why the requested block was not verified.
        cause: String,
        /// Recovery outcome for the original healthy block.
        recovery: String,
    },
}

/// Failure to read the experimental wired macro-table candidate.
#[derive(Debug, Error)]
pub enum WiredMacroReadError {
    /// Reads require the exact identity printed by the read-only wired model probe.
    #[error(
        "wired AULA macro reads require the exact identity from `openhid diag aula --wired-model-probe`"
    )]
    IdentityRequired,
    /// No matching direct USB keyboard collection was found.
    #[error("no AULA wired keyboard collection (258A:010C) was found")]
    NotFound,
    /// The supplied identity did not select exactly one matching collection.
    #[error("AULA wired collection selection failed: {0}")]
    Selection(&'static str),
    /// The selected collection did not pass the F87 Pro model query.
    #[error("AULA wired macro read refused because the model query failed: {0}")]
    ModelProbe(#[source] Box<WiredProbeError>),
    /// The selected HID Feature Report could not be opened or read.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// The complete eight-part query exceeded its deadline.
    #[error("AULA wired macro-table read timed out")]
    Timeout,
    /// The device returned a response that does not match the macro query.
    #[error("invalid AULA wired macro-table response: {0}")]
    InvalidResponse(&'static str),
}

/// A wired F87 Pro macro-table write was refused.
#[derive(Debug, Error)]
pub enum WiredMacroWriteError {
    /// The candidate read is unstable and does not provide a trustworthy backup.
    #[error(
        "AULA wired macro writes are disabled until the macro read protocol produces a stable, complete backup"
    )]
    ProtocolUnverified,
}

/// Raw 4096-byte result of the experimental wired macro-table read.
///
/// This is an opaque candidate dump. Its size and layout have not been
/// validated against the installed Windows application or an attached F87 Pro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiredMacroTable(Vec<u8>);

impl WiredMacroTable {
    /// Construct an opaque candidate table from exactly 4096 raw bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, InvalidWiredMacroTable> {
        if bytes.len() != MACRO_TABLE_LENGTH {
            return Err(InvalidWiredMacroTable::Length);
        }
        Ok(Self(bytes))
    }

    /// The eight concatenated 512-byte payloads returned by the device.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A raw candidate macro table does not contain exactly 4096 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidWiredMacroTable {
    /// The candidate table is not eight 512-byte blocks.
    #[error("an AULA wired macro table requires exactly 4096 bytes")]
    Length,
}

/// The wired F87 Pro Feature Report model-query response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiredModelProbe {
    response: [u8; MODEL_QUERY_RESPONSE_LENGTH],
    identity: String,
}

impl WiredModelProbe {
    /// The validated 14-byte response, including its report ID and command.
    #[must_use]
    pub const fn response(&self) -> &[u8; MODEL_QUERY_RESPONSE_LENGTH] {
        &self.response
    }

    /// Exact runtime identity of the HID collection that returned this response.
    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

/// Failure to query a wired AULA candidate through Feature Report `0x06`.
#[derive(Debug, Error)]
pub enum WiredProbeError {
    /// No matching direct USB keyboard collection was found.
    #[error("no AULA wired keyboard collection (258A:010C) was found")]
    NotFound,
    /// The supplied runtime identity did not select one collection.
    #[error("AULA wired collection selection failed: {0}")]
    Selection(&'static str),
    /// A backend could not open or communicate with the feature collection.
    #[error(transparent)]
    Backend(#[from] BackendError),
    /// The complete query exceeded its deadline.
    #[error("AULA wired model query timed out")]
    Timeout,
    /// The device returned a response that does not match the model query.
    #[error("invalid AULA wired model-query response: {0}")]
    InvalidResponse(&'static str),
}

/// Failure to upload one complete wired AULA keymap layer.
#[derive(Debug, Error)]
pub enum WiredKeymapWriteError {
    /// Writes require the exact identity printed by the read-only wired model probe.
    #[error(
        "wired AULA keymap writes require the exact identity from `openhid diag aula --wired-model-probe`"
    )]
    IdentityRequired,
    /// No matching direct USB keyboard collection was found.
    #[error("no AULA wired keyboard collection (258A:010C) was found")]
    NotFound,
    /// The supplied identity did not select exactly one matching collection.
    #[error("AULA wired collection selection failed: {0}")]
    Selection(&'static str),
    /// The supplied report did not pass the wired keymap format validation.
    #[error(transparent)]
    InvalidBlob(#[from] InvalidAulaWiredKeymapBlob),
    /// The selected collection did not pass the read-only F87-family model query.
    #[error("AULA wired keymap upload refused because the model query failed: {0}")]
    ModelProbe(#[source] Box<WiredProbeError>),
    /// The selected HID Feature Report could not be opened or written.
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// Upload one complete validated keymap layer to a specifically selected wired F87 Pro.
///
/// A read-only model query must first succeed on the exact collection. The
/// report is then sent once as Feature Report `0x06`. The documented F87 path
/// does not provide reliable keymap readback, so success only confirms that
/// the host HID API accepted the transfer; it does not verify device behavior.
pub async fn write_wired_keymap(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    blob: &AulaWiredKeymapBlob,
) -> Result<(), WiredKeymapWriteError> {
    let identity = identity.ok_or(WiredKeymapWriteError::IdentityRequired)?;
    let nodes = backend.enumerate().await?;
    let has_wired_candidate = nodes.iter().any(|node| {
        openhid_device_registry::aula::matches_wired_candidate(node.vendor_id, node.product_id)
    });
    let candidates: Vec<_> = nodes
        .into_iter()
        .filter(|node| {
            openhid_device_registry::aula::matches_wired_candidate(node.vendor_id, node.product_id)
                && wired_identity_matches(&node.identity(), identity)
        })
        .collect();
    if candidates.is_empty() {
        if !has_wired_candidate {
            return Err(WiredKeymapWriteError::NotFound);
        }
        return Err(WiredKeymapWriteError::Selection(
            "no 258A:010C collection matched the supplied identity",
        ));
    }
    let [node] = candidates.as_slice() else {
        return Err(WiredKeymapWriteError::Selection(
            "multiple 258A:010C collections matched the supplied identity",
        ));
    };

    // Re-validate at the I/O boundary, even though the current caller carries
    // the validated core type. This keeps the transport invariant explicit.
    AulaWiredKeymapBlob::from_bytes(blob.as_bytes().to_vec())?;
    query_candidate(backend, node)
        .await
        .map_err(|error| WiredKeymapWriteError::ModelProbe(Box::new(error)))?;
    let mut session = backend.open_feature_session(node).await?;
    session.write_feature_report(blob.as_bytes()).await?;
    Ok(())
}

/// Read the complete validated config block from one exactly selected wired F87 Pro.
pub async fn read_wired_config(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WiredConfigBlock, WiredConfigReadError> {
    let identity = identity.ok_or(WiredConfigReadError::IdentityRequired)?;
    let node = select_wired_node(backend, identity).await?;
    query_candidate(backend, &node)
        .await
        .map_err(|error| WiredConfigReadError::ModelProbe(Box::new(error)))?;
    read_config_on_node(backend, &node).await
}

/// Experimentally read a 4096-byte macro-table candidate over the wired F87 Pro feature channel.
///
/// The route must match exactly one `258A:010C` collection and pass the
/// read-only F87 Pro model query. The request uses candidate `0x06/0x85`
/// feature reads and performs no macro writes. The response is returned
/// opaque; neither its table shape nor its completeness is established.
pub async fn read_wired_macro_table(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WiredMacroTable, WiredMacroReadError> {
    let identity = identity.ok_or(WiredMacroReadError::IdentityRequired)?;
    let node = select_wired_node(backend, identity)
        .await
        .map_err(map_macro_selection_error)?;
    query_candidate(backend, &node)
        .await
        .map_err(|error| WiredMacroReadError::ModelProbe(Box::new(error)))?;

    tokio::time::timeout(MACRO_READ_TIMEOUT, read_macro_table_on_node(backend, &node))
        .await
        .map_err(|_| WiredMacroReadError::Timeout)?
}

/// Refuse wired macro-table writes until the protocol is verified against the OEM application.
///
/// The current read candidate is unstable on the attached F87 Pro, and static
/// analysis of the OEM driver shows a different logical transfer size. Keeping
/// this guard at the device boundary ensures no experimental macro report can
/// reach hardware through any caller.
pub async fn write_wired_macro_table(
    _backend: &dyn HidBackend,
    _identity: Option<&str>,
    _expected_original: &WiredMacroTable,
    _target: &WiredMacroTable,
) -> Result<(), WiredMacroWriteError> {
    Err(WiredMacroWriteError::ProtocolUnverified)
}

async fn read_macro_table_on_node(
    backend: &dyn HidBackend,
    node: &NodeInfo,
) -> Result<WiredMacroTable, WiredMacroReadError> {
    let mut session = backend.open_feature_session(node).await?;
    let mut table = Vec::with_capacity(MACRO_TABLE_LENGTH);
    for chunk in 0..MACRO_READ_CHUNKS {
        let mut request = [0; FEATURE_REPORT_LENGTH];
        request[..8].copy_from_slice(&[
            FEATURE_REPORT_ID,
            MACRO_READ_COMMAND,
            0,
            0,
            MACRO_READ_CHUNKS,
            chunk,
            0,
            2,
        ]);
        session.write_feature_report(&request).await?;

        let mut response = [0; FEATURE_REPORT_LENGTH];
        response[0] = FEATURE_REPORT_ID;
        let length = session.read_feature_report(&mut response).await?;
        if length < FEATURE_REPORT_LENGTH {
            return Err(WiredMacroReadError::InvalidResponse(
                "response is shorter than the 8-byte header and 512-byte payload",
            ));
        }
        if response[..8] != request[..8] {
            return Err(WiredMacroReadError::InvalidResponse(
                "response header does not match the macro read query",
            ));
        }
        table.extend_from_slice(&response[CONFIG_HEADER_LENGTH..FEATURE_REPORT_LENGTH]);
    }
    WiredMacroTable::from_bytes(table).map_err(|_| {
        WiredMacroReadError::InvalidResponse("macro-table response did not contain 4096 bytes")
    })
}

fn map_macro_selection_error(error: WiredConfigReadError) -> WiredMacroReadError {
    match error {
        WiredConfigReadError::IdentityRequired => WiredMacroReadError::IdentityRequired,
        WiredConfigReadError::NotFound => WiredMacroReadError::NotFound,
        WiredConfigReadError::Selection(reason) => WiredMacroReadError::Selection(reason),
        WiredConfigReadError::ModelProbe(source) => WiredMacroReadError::ModelProbe(source),
        WiredConfigReadError::Backend(error) => WiredMacroReadError::Backend(error),
        WiredConfigReadError::InvalidResponse(_) | WiredConfigReadError::InvalidBlock(_) => {
            WiredMacroReadError::Selection("wired route selection failed")
        }
    }
}

/// Replace the complete wired config block and verify it, restoring the
/// previous healthy block if the write or readback fails.
pub async fn write_wired_config(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    block: &WiredConfigBlock,
) -> Result<WiredConfigBlock, WiredConfigWriteError> {
    let identity = identity.ok_or(WiredConfigWriteError::IdentityRequired)?;
    // Re-validate at the I/O boundary to keep the healthy-block invariant local.
    let target = WiredConfigBlock::from_bytes(block.as_bytes())?;
    let node = select_wired_node(backend, identity)
        .await
        .map_err(|error| match error {
            WiredConfigReadError::IdentityRequired => WiredConfigWriteError::IdentityRequired,
            WiredConfigReadError::NotFound => WiredConfigWriteError::NotFound,
            WiredConfigReadError::Selection(reason) => WiredConfigWriteError::Selection(reason),
            WiredConfigReadError::ModelProbe(source) => WiredConfigWriteError::ModelProbe(source),
            other => WiredConfigWriteError::ReadOriginal(Box::new(other)),
        })?;
    query_candidate(backend, &node)
        .await
        .map_err(|error| WiredConfigWriteError::ModelProbe(Box::new(error)))?;
    let original = read_config_on_node(backend, &node)
        .await
        .map_err(|error| WiredConfigWriteError::ReadOriginal(Box::new(error)))?;

    let cause = match write_config_on_node(backend, &node, &target).await {
        Ok(()) => {
            tokio::time::sleep(CONFIG_WRITE_SETTLE).await;
            match read_config_on_node(backend, &node).await {
                Ok(actual) if actual == target => return Ok(actual),
                Ok(_) => "device readback differs from the requested config block".to_owned(),
                Err(error) => format!("device readback failed: {error}"),
            }
        }
        Err(error) => {
            // A failed host transfer can still have reached the keyboard.
            tokio::time::sleep(CONFIG_WRITE_SETTLE).await;
            format!("feature report write failed: {error}")
        }
    };

    let recovery = match write_config_on_node(backend, &node, &original).await {
        Ok(()) => {
            tokio::time::sleep(CONFIG_WRITE_SETTLE).await;
            match read_config_on_node(backend, &node).await {
                Ok(actual) if actual == original => "verified".to_owned(),
                Ok(_) => "failed: restored block readback differs".to_owned(),
                Err(error) => format!("failed: restored block readback failed: {error}"),
            }
        }
        Err(error) => format!("failed: restoring original block failed: {error}"),
    };
    Err(WiredConfigWriteError::WriteRecovery { cause, recovery })
}

pub(super) fn wired_identity_matches(node_identity: &str, requested: &str) -> bool {
    node_identity == requested
        || canonical_wired_identity(node_identity) == canonical_wired_identity(requested)
}

fn canonical_wired_identity(value: &str) -> String {
    let value = value.trim();
    let path = value
        .strip_prefix("id:UncPath(")
        .and_then(|value| value.strip_suffix(')'))
        .map_or(value, str::trim)
        .trim_matches('"');
    // Windows' native argument parser can consume one leading slash when a
    // copied `UncPath("...")` identity crosses a process. Ignore the number
    // of leading slashes so both spellings address the same case-insensitive
    // HID path.
    path.trim_start_matches('\\').to_ascii_lowercase()
}

async fn select_wired_node(
    backend: &dyn HidBackend,
    identity: &str,
) -> Result<NodeInfo, WiredConfigReadError> {
    let nodes = backend.enumerate().await?;
    let has_candidate = nodes.iter().any(|node| {
        openhid_device_registry::aula::matches_wired_candidate(node.vendor_id, node.product_id)
    });
    let mut candidates = nodes.into_iter().filter(|node| {
        openhid_device_registry::aula::matches_wired_candidate(node.vendor_id, node.product_id)
            && wired_identity_matches(&node.identity(), identity)
    });
    let Some(node) = candidates.next() else {
        return if has_candidate {
            Err(WiredConfigReadError::Selection(
                "no 258A:010C collection matched the supplied identity",
            ))
        } else {
            Err(WiredConfigReadError::NotFound)
        };
    };
    if candidates.next().is_some() {
        return Err(WiredConfigReadError::Selection(
            "multiple 258A:010C collections matched the supplied identity",
        ));
    }
    Ok(node)
}

async fn read_config_on_node(
    backend: &dyn HidBackend,
    node: &NodeInfo,
) -> Result<WiredConfigBlock, WiredConfigReadError> {
    let mut request = [0; FEATURE_REPORT_LENGTH];
    request[0] = FEATURE_REPORT_ID;
    request[1] = CONFIG_READ_COMMAND;
    request[4] = 0x01;
    request[6..8].copy_from_slice(&AULA_WIRED_CONFIG_BLOCK_LENGTH.to_le_bytes());
    let mut session = backend.open_feature_session(node).await?;
    session.write_feature_report(&request).await?;

    let mut response = [0; FEATURE_REPORT_LENGTH];
    response[0] = FEATURE_REPORT_ID;
    let length = session.read_feature_report(&mut response).await?;
    if length < CONFIG_TRANSFER_LENGTH {
        return Err(WiredConfigReadError::InvalidResponse(
            "response is shorter than the 8-byte header and 128-byte block",
        ));
    }
    if response[0] != FEATURE_REPORT_ID
        || response[1] != CONFIG_READ_COMMAND
        || response[2..6] != request[2..6]
        || response[6..8] != request[6..8]
    {
        return Err(WiredConfigReadError::InvalidResponse(
            "response header does not match the config read query",
        ));
    }
    WiredConfigBlock::from_bytes(&response[CONFIG_HEADER_LENGTH..CONFIG_TRANSFER_LENGTH])
        .map_err(WiredConfigReadError::from)
}

async fn write_config_on_node(
    backend: &dyn HidBackend,
    node: &NodeInfo,
    block: &WiredConfigBlock,
) -> Result<(), BackendError> {
    let mut report = [0; FEATURE_REPORT_LENGTH];
    report[0] = FEATURE_REPORT_ID;
    report[1] = CONFIG_WRITE_COMMAND;
    report[4] = 0x01;
    report[6..8].copy_from_slice(&AULA_WIRED_CONFIG_BLOCK_LENGTH.to_le_bytes());
    report[CONFIG_HEADER_LENGTH..CONFIG_TRANSFER_LENGTH].copy_from_slice(block.as_bytes());
    let mut session = backend.open_feature_session(node).await?;
    session.write_feature_report(&report).await
}

/// Query candidate `258A:010C` collections using the read-only `0x06/0x82` request.
///
/// The wired implementation uses a Feature Report rather than the wireless
/// receiver's `0x13` output reports. When `identity` is omitted, each matching
/// collection is queried and exactly one F87 Pro response is required. No
/// lighting, keymap, or other settings are written.
pub async fn read_wired_model_probe(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<WiredModelProbe, WiredProbeError> {
    tokio::time::timeout(MODEL_QUERY_TIMEOUT, async {
        let candidates = wired_candidates(backend, identity).await?;
        let mut valid_responses = Vec::new();
        let mut last_error = None;

        for node in candidates {
            match query_candidate(backend, &node).await {
                Ok(response) => valid_responses.push(response),
                Err(error) => last_error = Some(error),
            }
        }

        match valid_responses.as_slice() {
            [response] => Ok(response.clone()),
            [] => Err(last_error.unwrap_or(WiredProbeError::InvalidResponse(
                "no candidate collection returned report 0x06 command 0x82",
            ))),
            _ => Err(WiredProbeError::Selection(
                "multiple collections returned valid model-query responses; specify --identity",
            )),
        }
    })
    .await
    .map_err(|_| WiredProbeError::Timeout)?
}

/// Discover only wired `258A:010C` collections that pass the F87 Pro model
/// query. Invalid, unrelated, and non-responsive collections are omitted.
pub async fn discover_wired_f87_nodes(
    backend: &dyn HidBackend,
    nodes: &[NodeInfo],
) -> Result<Vec<NodeInfo>, WiredProbeError> {
    let candidates: Vec<_> = nodes
        .iter()
        .filter(|node| {
            openhid_device_registry::aula::matches_wired_candidate(node.vendor_id, node.product_id)
                && node.usage_page == 0xff00
        })
        .collect();
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    tokio::time::timeout(MODEL_QUERY_TIMEOUT, async {
        let mut verified = Vec::new();
        for node in candidates {
            if query_candidate(backend, node).await.is_ok() {
                verified.push(node.clone());
            }
        }
        verified
    })
    .await
    .map_err(|_| WiredProbeError::Timeout)
}

async fn wired_candidates(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<Vec<NodeInfo>, WiredProbeError> {
    let nodes = backend.enumerate().await?;
    let candidates: Vec<_> = nodes
        .into_iter()
        .filter(|node| {
            openhid_device_registry::aula::matches_wired_candidate(node.vendor_id, node.product_id)
        })
        .collect();

    if candidates.is_empty() {
        return Err(WiredProbeError::NotFound);
    }

    if let Some(identity) = identity {
        let selected: Vec<_> = candidates
            .into_iter()
            .filter(|node| wired_identity_matches(&node.identity(), identity))
            .collect();
        return match selected.len() {
            1 => Ok(selected),
            0 => Err(WiredProbeError::Selection(
                "no 258A:010C collection matched the supplied identity",
            )),
            _ => Err(WiredProbeError::Selection(
                "multiple 258A:010C collections matched the supplied identity",
            )),
        };
    }

    Ok(candidates)
}

async fn query_candidate(
    backend: &dyn HidBackend,
    node: &NodeInfo,
) -> Result<WiredModelProbe, WiredProbeError> {
    let mut request = [0; FEATURE_REPORT_LENGTH];
    request[0] = FEATURE_REPORT_ID;
    request[1] = MODEL_QUERY_COMMAND;
    request[2] = 0x01;
    request[4] = 0x01;
    request[6] = 0x06;

    let mut session = backend.open_feature_session(node).await?;
    session.write_feature_report(&request).await?;

    let mut response = [0; FEATURE_REPORT_LENGTH];
    response[0] = FEATURE_REPORT_ID;
    let length = session.read_feature_report(&mut response).await?;
    if length < MODEL_QUERY_RESPONSE_LENGTH {
        return Err(WiredProbeError::InvalidResponse(
            "response is shorter than 14 bytes",
        ));
    }
    if response[0] != FEATURE_REPORT_ID || response[1] != MODEL_QUERY_COMMAND {
        return Err(WiredProbeError::InvalidResponse(
            "report ID or command does not match the query",
        ));
    }
    if response[2..7] != request[2..7] {
        return Err(WiredProbeError::InvalidResponse(
            "response does not echo the model-query selector",
        ));
    }
    if response[8] != MODEL_FAMILY_CODE {
        return Err(WiredProbeError::InvalidResponse(
            "response does not identify the supported AULA model family",
        ));
    }
    if response[MODEL_CODE_OFFSET] != F87_PRO_MODEL_CODE {
        return Err(WiredProbeError::InvalidResponse(
            "response does not identify the AULA F87 Pro wired model",
        ));
    }

    let mut probe = [0; MODEL_QUERY_RESPONSE_LENGTH];
    probe.copy_from_slice(&response[..MODEL_QUERY_RESPONSE_LENGTH]);
    Ok(WiredModelProbe {
        response: probe,
        identity: node.identity(),
    })
}
