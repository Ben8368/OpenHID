//! F87 PRO request policy: allow verified writes and recovery to finish.

use crate::AgentClient;
use openhid_core::hid::aula::{LightingChange, LightingMode, LightingState};
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
