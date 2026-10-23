//! F87 PRO request policy: allow verified writes and recovery to finish.

use crate::AgentClient;
use openhid_core::hid::aula::{AulaPerKeyRgb, LightingChange, LightingMode, LightingState};
use openhid_core::hid::{DeviceRoute, WriteError};
use std::time::{Duration, Instant};
use tarpc::{
    client::RpcError,
    context::{self, Context},
};

// This is an IPC response budget, not a protocol or native-I/O deadline.
// The agent's owned transaction can still recover after the caller disconnects.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(90);

fn request_context() -> Context {
    let mut context = context::current();
    context.deadline = Instant::now() + RESPONSE_TIMEOUT;
    context
}

/// Read the F87 PRO under the shared transaction response policy.
pub async fn read_lighting(
    client: &AgentClient,
    route: DeviceRoute,
) -> Result<Result<LightingState, WriteError>, RpcError> {
    client.read_aula_lighting(request_context(), route).await
}

/// Apply/save/preview without expiring the RPC before verified recovery.
pub async fn set_lighting(
    client: &AgentClient,
    route: DeviceRoute,
    change: LightingChange,
    mode: LightingMode,
) -> Result<Result<LightingState, WriteError>, RpcError> {
    client
        .set_aula_lighting(request_context(), route, change, mode)
        .await
}

/// Read all currently supported F87 PRO settings.
pub async fn read_settings(
    client: &AgentClient,
    route: DeviceRoute,
) -> Result<Result<openhid_core::hid::aula::AulaState, WriteError>, RpcError> {
    client.read_aula_settings(request_context(), route).await
}

/// Apply/save/preview verified F87 PRO settings.
pub async fn set_settings(
    client: &AgentClient,
    route: DeviceRoute,
    change: openhid_core::hid::aula::AulaChange,
    mode: LightingMode,
) -> Result<Result<openhid_core::hid::aula::AulaState, WriteError>, RpcError> {
    client
        .set_aula_settings(request_context(), route, change, mode)
        .await
}

/// Apply a complete per-key RGB table under the shared long-write response policy.
pub async fn set_per_key_rgb(
    client: &AgentClient,
    route: DeviceRoute,
    colors: AulaPerKeyRgb,
) -> Result<Result<openhid_core::hid::aula::AulaState, WriteError>, RpcError> {
    client
        .set_aula_per_key_rgb(request_context(), route, colors)
        .await
}

/// Read both complete raw wireless keymap layers.
pub async fn read_keymap(
    client: &AgentClient,
    route: DeviceRoute,
) -> Result<Result<openhid_core::hid::aula::AulaKeymap, WriteError>, RpcError> {
    client.read_aula_keymap(request_context(), route).await
}

/// Read the 126-entry base layer independently of the Fn layer.
pub async fn read_base_matrix(
    client: &AgentClient,
    route: DeviceRoute,
) -> Result<Result<openhid_core::hid::aula::AulaBaseMatrix, WriteError>, RpcError> {
    client.read_aula_base_matrix(request_context(), route).await
}

/// Read the incomplete wireless macro-memory prefix under the AULA response budget.
pub async fn read_macro_prefix(
    client: &AgentClient,
    route: DeviceRoute,
) -> Result<Result<Vec<u8>, WriteError>, RpcError> {
    client
        .read_aula_macro_prefix(request_context(), route)
        .await
}

/// Read the experimental OEM-sized wireless macro-table candidate.
///
/// Each 512-byte result is raw diagnostic data and is not a restorable backup.
pub async fn read_macro_table_candidate(
    client: &AgentClient,
    route: DeviceRoute,
) -> Result<Result<Vec<Vec<u8>>, WriteError>, RpcError> {
    client
        .read_aula_macro_table_candidate(request_context(), route)
        .await
}

/// Read one complete wired F87 Pro config block.
pub async fn read_wired_config(
    client: &AgentClient,
    route: DeviceRoute,
) -> Result<Result<Vec<u8>, WriteError>, RpcError> {
    client
        .read_aula_wired_config(request_context(), route)
        .await
}

/// Replace, verify, and recover a complete wired F87 Pro config block.
pub async fn set_wired_config(
    client: &AgentClient,
    route: DeviceRoute,
    bytes: Vec<u8>,
) -> Result<Result<Vec<u8>, WriteError>, RpcError> {
    client
        .set_aula_wired_config(request_context(), route, bytes)
        .await
}

/// Upload a model-gated wired F87 Pro keymap report.
pub async fn set_wired_keymap(
    client: &AgentClient,
    route: DeviceRoute,
    bytes: Vec<u8>,
) -> Result<Result<(), WriteError>, RpcError> {
    client
        .set_aula_wired_keymap(request_context(), route, bytes)
        .await
}

/// Apply both keymap layers under the shared long-write response policy.
pub async fn set_keymap(
    client: &AgentClient,
    route: DeviceRoute,
    keymap: openhid_core::hid::aula::AulaKeymap,
) -> Result<Result<openhid_core::hid::aula::AulaKeymap, WriteError>, RpcError> {
    client
        .set_aula_keymap(request_context(), route, keymap)
        .await
}

/// Apply a keymap only if it still matches the complete snapshot the caller edited.
pub async fn set_keymap_if_unchanged(
    client: &AgentClient,
    route: DeviceRoute,
    expected: openhid_core::hid::aula::AulaKeymap,
    keymap: openhid_core::hid::aula::AulaKeymap,
) -> Result<Result<openhid_core::hid::aula::AulaKeymap, WriteError>, RpcError> {
    client
        .set_aula_keymap_if_unchanged(request_context(), route, expected, keymap)
        .await
}
