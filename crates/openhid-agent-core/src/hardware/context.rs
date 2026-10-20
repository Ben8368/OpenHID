//! Shared ownership of the agent's hardware backend and host-I/O policy.

use std::sync::Arc;

use openhid_core::device::{LightState, StandaloneDevice};
use openhid_core::hid::{LightCommand, PairingError, WriteError};
use openhid_hid::backend::{HidBackend, HotplugStream};
use openhid_hid::inventory::persist::ProbeCacheStore;
use openhid_hid::{
    ChannelPool, DeviceIoGate, DeviceRoute, Enumerator, FileProbeCacheStore, InventoryError,
    LightWatch, LitraModel, PairingCommand, PairingEvent, ReceiverSelector,
};
use tokio::sync::mpsc;

/// One coherent source for every backend-dependent agent hardware service.
///
/// Production uses the process-wide native backend and device-I/O gate. Tests
/// and alternate hosts inject one backend identity and gate; their enumerator,
/// hotplug stream, standalone discovery, and channel pool all derive from it.
#[derive(Clone)]
pub struct HardwareContext {
    backend: Arc<dyn HidBackend>,
    device_io: DeviceIoGate,
    channel_pool: ChannelPool,
    probe_cache: Option<Arc<dyn ProbeCacheStore>>,
}

impl HardwareContext {
    /// Build the production context over this process's native backend and
    /// lifecycle gate, with the usual file-backed probe cache when available.
    #[must_use]
    pub fn production() -> Self {
        let probe_cache = FileProbeCacheStore::in_data_dir()
            .map(|store| Arc::new(store) as Arc<dyn ProbeCacheStore>);
        Self::from_parts(
            openhid_hid::host::backend(),
            openhid_hid::host::device_io_gate(),
            probe_cache,
        )
    }

    /// Build an injected, memory-cache-only context over `backend` and
    /// `device_io`.
    #[must_use]
    pub fn injected(backend: Arc<dyn HidBackend>, device_io: DeviceIoGate) -> Self {
        Self::from_parts(backend, device_io, None)
    }

    /// Supply an explicit persistent probe-cache store.
    #[must_use]
    pub fn with_probe_cache(mut self, store: Arc<dyn ProbeCacheStore>) -> Self {
        self.probe_cache = Some(store);
        self
    }

    /// A read capability for the shared host-lifecycle gate.
    #[must_use]
    pub fn device_io(&self) -> DeviceIoGate {
        self.device_io.clone()
    }

    /// The shared route-opening channel pool derived from this context's
    /// backend.
    #[must_use]
    pub fn channel_pool(&self) -> ChannelPool {
        self.channel_pool.clone()
    }

    /// Apply one explicit semantic command to a standalone light through this
    /// context's backend and lifecycle gate.
    pub async fn apply_light(
        &self,
        route: &DeviceRoute,
        command: LightCommand,
    ) -> Result<(), WriteError> {
        super::light::cancel_light_reapply(route);
        super::light::apply_light(self, route, command).await
    }

    /// Read the verified F87 PRO settings through this context's backend.
    pub async fn read_aula_state(
        &self,
        route: &DeviceRoute,
    ) -> Result<openhid_core::hid::aula::AulaState, WriteError> {
        let identity =
            openhid_core::hid::aula::route_identity(route).ok_or(WriteError::DeviceNotFound)?;
        self.device_io
            .ensure_allowed()
            .map_err(|_| WriteError::DeviceNotFound)?;
        openhid_hid::aula::read_selected_config(&*self.backend, Some(identity))
            .await
            .map(|config| config.aula_state())
            .map_err(|error| WriteError::Hid(error.to_string()))
    }

    /// Compatibility lighting view of the shared, verified settings snapshot.
    pub async fn read_aula_lighting(
        &self,
        route: &DeviceRoute,
    ) -> Result<openhid_core::hid::aula::LightingState, WriteError> {
        self.read_aula_state(route)
            .await
            .map(|state| state.lighting)
    }

    /// Apply/save/preview verified settings in a worker that owns recovery.
    /// Dropping the requesting RPC signals cancellation while recovery continues.
    pub async fn set_aula_settings(
        &self,
        route: &DeviceRoute,
        change: openhid_core::hid::aula::AulaChange,
        mode: openhid_core::hid::aula::LightingMode,
    ) -> Result<openhid_core::hid::aula::AulaState, WriteError> {
        let identity =
            openhid_core::hid::aula::route_identity(route).ok_or(WriteError::DeviceNotFound)?;
        openhid_hid::host::start_aula_settings_on(
            Arc::clone(&self.backend),
            self.device_io.clone(),
            Some(identity),
            change,
            mode,
        )
        .await?
        .finish()
        .await
        .map(|result| result.state)
    }

    /// Lighting-only requests share settings validation, worker and recovery.
    pub async fn set_aula_lighting(
        &self,
        route: &DeviceRoute,
        change: openhid_core::hid::aula::LightingChange,
        mode: openhid_core::hid::aula::LightingMode,
    ) -> Result<openhid_core::hid::aula::LightingState, WriteError> {
        let change = openhid_core::hid::aula::AulaChange::new(Some(change), None)
            .map_err(|error| WriteError::Hid(error.to_string()))?;
        self.set_aula_settings(route, change, mode)
            .await
            .map(|state| state.lighting)
    }

    pub(crate) fn set_light_in_background(
        &self,
        target: Option<DeviceRoute>,
        light: &openhid_core::config::LightSettings,
        capabilities: openhid_core::device::LightCapabilities,
    ) {
        super::light::set_light_in_background(self, target, light, capabilities);
    }

    pub(crate) fn enumerator(&self) -> Enumerator {
        let enumerator = Enumerator::with_backend(Arc::clone(&self.backend));
        match &self.probe_cache {
            Some(store) => enumerator.with_probe_cache(Arc::clone(store)),
            None => enumerator,
        }
    }

    pub(crate) fn watch_hotplug(&self) -> Result<HotplugStream, InventoryError> {
        openhid_hid::inventory::hotplug::watch_hotplug(&*self.backend)
    }

    pub(crate) async fn enumerate_standalone(
        &self,
    ) -> Result<Vec<StandaloneDevice>, InventoryError> {
        openhid_hid::inventory::standalone::enumerate_standalone(&*self.backend).await
    }

    pub(crate) async fn run_pairing(
        &self,
        target: ReceiverSelector,
        commands: mpsc::UnboundedReceiver<PairingCommand>,
        events: mpsc::UnboundedSender<PairingEvent>,
    ) -> Result<(), PairingError> {
        openhid_hid::pairing::run_pairing(&*self.backend, target, commands, events).await
    }

    /// Remove the device `route` reaches from its receiver's pairing table.
    pub async fn unpair(&self, route: &DeviceRoute) -> Result<(), PairingError> {
        openhid_hid::pairing::unpair(&*self.backend, route).await
    }

    pub(super) async fn apply_litra(
        &self,
        route: &DeviceRoute,
        model: LitraModel,
        command: LightCommand,
    ) -> Result<Option<LightState>, WriteError> {
        openhid_hid::write::apply_litra(&*self.backend, route, model, command).await
    }

    /// Watch the Litra `route` reaches for changes made on the light itself
    /// (`None` when only its raw report reaches it).
    pub(crate) async fn watch_litra(
        &self,
        route: &DeviceRoute,
        model: LitraModel,
    ) -> Result<Option<LightWatch>, WriteError> {
        openhid_hid::write::watch_litra(&*self.backend, route, model).await
    }

    fn from_parts(
        backend: Arc<dyn HidBackend>,
        device_io: DeviceIoGate,
        probe_cache: Option<Arc<dyn ProbeCacheStore>>,
    ) -> Self {
        let channel_pool = ChannelPool::with_backend(Arc::clone(&backend));
        Self {
            backend,
            device_io,
            channel_pool,
            probe_cache,
        }
    }
}

#[cfg(test)]
#[path = "context/corpus_tests.rs"]
mod corpus_tests;

#[cfg(test)]
#[path = "context/aula_tests.rs"]
mod aula_tests;
