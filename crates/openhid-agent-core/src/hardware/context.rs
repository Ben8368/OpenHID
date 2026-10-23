//! Shared ownership of the agent's hardware backend and host-I/O policy.

use std::{future::Future, sync::Arc};

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
        self.read_aula_locked(route, |backend, identity| async move {
            openhid_hid::aula::read_selected_state(&*backend, Some(&identity)).await
        })
        .await
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

    /// Apply a complete per-key RGB table, then select and verify custom mode.
    pub async fn set_aula_per_key_rgb(
        &self,
        route: &DeviceRoute,
        colors: openhid_core::hid::aula::AulaPerKeyRgb,
    ) -> Result<openhid_core::hid::aula::AulaState, WriteError> {
        let identity =
            openhid_core::hid::aula::route_identity(route).ok_or(WriteError::DeviceNotFound)?;
        openhid_hid::host::start_aula_per_key_rgb_on(
            Arc::clone(&self.backend),
            self.device_io.clone(),
            Some(identity),
            colors,
        )
        .await?
        .finish()
        .await
    }

    /// Read both validated raw keymap layers through this context's backend.
    pub async fn read_aula_keymap(
        &self,
        route: &DeviceRoute,
    ) -> Result<openhid_core::hid::aula::AulaKeymap, WriteError> {
        self.read_aula_locked(route, |backend, identity| async move {
            openhid_hid::aula::read_selected_keymap(&*backend, Some(&identity)).await
        })
        .await
    }

    /// Read the validated base layer independently of the unverified Fn selector.
    pub async fn read_aula_base_matrix(
        &self,
        route: &DeviceRoute,
    ) -> Result<openhid_core::hid::aula::AulaBaseMatrix, WriteError> {
        self.read_aula_locked(route, |backend, identity| async move {
            openhid_hid::aula::read_selected_base_matrix(&*backend, Some(&identity)).await
        })
        .await
    }

    /// Read the experimental F87 PRO macro-memory prefix through this
    /// context's serialized device-I/O path.
    pub async fn read_aula_macro_prefix(&self, route: &DeviceRoute) -> Result<Vec<u8>, WriteError> {
        self.read_aula_locked(route, |backend, identity| async move {
            openhid_hid::aula::read_selected_macro_prefix(&*backend, Some(&identity))
                .await
                .map(|prefix| prefix.bytes().to_vec())
        })
        .await
    }

    /// Read the experimental OEM-sized wireless macro-table candidate through
    /// this context's serialized device-I/O path.
    pub async fn read_aula_macro_table_candidate(
        &self,
        route: &DeviceRoute,
    ) -> Result<Vec<Vec<u8>>, WriteError> {
        self.read_aula_locked(route, |backend, identity| async move {
            openhid_hid::aula::read_selected_macro_table_candidate(&*backend, Some(&identity))
                .await
                .map(|candidate| candidate.reads().to_vec())
        })
        .await
    }

    /// Read a complete healthy wired F87 Pro config block through the
    /// per-route Agent I/O queue.
    pub async fn read_aula_wired_config(&self, route: &DeviceRoute) -> Result<Vec<u8>, WriteError> {
        let identity = openhid_core::hid::aula::wired_route_identity(route)
            .ok_or(WriteError::DeviceNotFound)?
            .to_owned();
        let backend = Arc::clone(&self.backend);
        let device_io = self.device_io.clone();
        openhid_hid::lighting::LightingJob::spawn(route, move |_cancel| async move {
            device_io
                .ensure_allowed()
                .map_err(|_| WriteError::DeviceNotFound)?;
            openhid_hid::aula::read_wired_config(&*backend, Some(&identity))
                .await
                .map(|block| block.as_bytes().to_vec())
                .map_err(|error| WriteError::Hid(error.to_string()))
        })?
        .finish()
        .await
    }

    /// Write and verify a full wired config block in an owned per-route Agent
    /// worker so snapshot recovery continues if the requesting client leaves.
    pub async fn set_aula_wired_config(
        &self,
        route: &DeviceRoute,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>, WriteError> {
        let identity = openhid_core::hid::aula::wired_route_identity(route)
            .ok_or(WriteError::DeviceNotFound)?
            .to_owned();
        let block = openhid_hid::aula::WiredConfigBlock::from_bytes(&bytes)
            .map_err(|error| WriteError::Hid(error.to_string()))?;
        let backend = Arc::clone(&self.backend);
        let device_io = self.device_io.clone();
        openhid_hid::lighting::LightingJob::spawn(route, move |_cancel| async move {
            device_io
                .ensure_allowed()
                .map_err(|_| WriteError::DeviceNotFound)?;
            openhid_hid::aula::write_wired_config(&*backend, Some(&identity), &block)
                .await
                .map(|block| block.as_bytes().to_vec())
                .map_err(|error| WriteError::Hid(error.to_string()))
        })?
        .finish()
        .await
    }

    /// Upload one wired F87 Pro keymap report through the per-route Agent
    /// worker after validating its format and exact wired device identity.
    pub async fn set_aula_wired_keymap(
        &self,
        route: &DeviceRoute,
        bytes: Vec<u8>,
    ) -> Result<(), WriteError> {
        let identity = openhid_core::hid::aula::wired_route_identity(route)
            .ok_or(WriteError::DeviceNotFound)?
            .to_owned();
        let blob = openhid_core::hid::aula::AulaWiredKeymapBlob::from_bytes(bytes)
            .map_err(|error| WriteError::Hid(error.to_string()))?;
        let backend = Arc::clone(&self.backend);
        let device_io = self.device_io.clone();
        openhid_hid::lighting::LightingJob::spawn(route, move |_cancel| async move {
            device_io
                .ensure_allowed()
                .map_err(|_| WriteError::DeviceNotFound)?;
            openhid_hid::aula::write_wired_keymap(&*backend, Some(&identity), &blob)
                .await
                .map_err(|error| WriteError::Hid(error.to_string()))
        })?
        .finish()
        .await
    }

    async fn read_aula_locked<T, Operation, OperationFuture>(
        &self,
        route: &DeviceRoute,
        operation: Operation,
    ) -> Result<T, WriteError>
    where
        T: Send + 'static,
        Operation: FnOnce(Arc<dyn HidBackend>, String) -> OperationFuture + Send + 'static,
        OperationFuture: Future<Output = Result<T, openhid_hid::aula::ReadError>> + Send + 'static,
    {
        let identity = openhid_core::hid::aula::route_identity(route)
            .ok_or(WriteError::DeviceNotFound)?
            .to_owned();
        let backend = Arc::clone(&self.backend);
        let device_io = self.device_io.clone();
        openhid_hid::lighting::LightingJob::spawn(route, move |_cancel| async move {
            device_io
                .ensure_allowed()
                .map_err(|_| WriteError::DeviceNotFound)?;
            operation(backend, identity)
                .await
                .map_err(|error| WriteError::Hid(error.to_string()))
        })?
        .finish()
        .await
    }

    /// Apply a complete keymap with owned cancellation and exact restoration.
    pub async fn set_aula_keymap(
        &self,
        route: &DeviceRoute,
        keymap: openhid_core::hid::aula::AulaKeymap,
    ) -> Result<openhid_core::hid::aula::AulaKeymap, WriteError> {
        let identity =
            openhid_core::hid::aula::route_identity(route).ok_or(WriteError::DeviceNotFound)?;
        openhid_hid::host::start_aula_keymap_on(
            Arc::clone(&self.backend),
            self.device_io.clone(),
            Some(identity),
            keymap,
        )
        .await?
        .finish()
        .await
    }

    /// Apply a keymap only if it still matches the complete snapshot the caller edited.
    pub async fn set_aula_keymap_if_unchanged(
        &self,
        route: &DeviceRoute,
        expected: openhid_core::hid::aula::AulaKeymap,
        keymap: openhid_core::hid::aula::AulaKeymap,
    ) -> Result<openhid_core::hid::aula::AulaKeymap, WriteError> {
        let identity =
            openhid_core::hid::aula::route_identity(route).ok_or(WriteError::DeviceNotFound)?;
        openhid_hid::host::start_aula_keymap_if_unchanged_on(
            Arc::clone(&self.backend),
            self.device_io.clone(),
            Some(identity),
            expected,
            keymap,
        )
        .await?
        .finish()
        .await
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
