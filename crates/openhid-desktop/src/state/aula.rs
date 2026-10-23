//! F87 PRO commands use device-owned memory, independently of HID++ RGB config.

use openhid_core::hid::aula::{
    AulaChange, AulaKeymap, AulaPerKeyRgb, AulaState, AulaWiredConfigBlock, AulaWiredKeymapBlob,
    LightingMode, route_identity, wired_route_identity,
};
use openhid_core::hid::{DeviceRoute, WriteError};
use tokio::sync::oneshot;

use super::AppState;
use crate::services::ipc::{
    ReadAulaKeymap, ReadAulaSettings, ReadAulaWiredConfig, SetAulaKeymapIfUnchanged,
    SetAulaPerKeyRgb, SetAulaSettings, SetAulaWiredConfig, SetAulaWiredKeymap,
};

impl AppState {
    fn aula_target(&self, route: &DeviceRoute) -> Result<(), WriteError> {
        if (route_identity(route).is_none() && wired_route_identity(route).is_none())
            || !self
                .current_record()
                .is_some_and(|record| record.online && record.route.as_ref() == Some(route))
        {
            return Err(WriteError::DeviceNotFound);
        }
        Ok(())
    }

    /// Read the complete wired F87 Pro settings block through the agent.
    pub(crate) fn read_aula_wired_config(
        &self,
        route: DeviceRoute,
    ) -> oneshot::Receiver<Result<AulaWiredConfigBlock, WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(ReadAulaWiredConfig { route, reply });
        }
        receiver
    }

    /// Replace the wired F87 Pro settings block through the agent.
    pub(crate) fn set_aula_wired_config(
        &self,
        route: DeviceRoute,
        block: AulaWiredConfigBlock,
    ) -> oneshot::Receiver<Result<AulaWiredConfigBlock, WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(SetAulaWiredConfig {
                route,
                block,
                reply,
            });
        }
        receiver
    }

    /// Upload one validated wired F87 Pro keymap report through the agent.
    pub(crate) fn set_aula_wired_keymap(
        &self,
        route: DeviceRoute,
        blob: AulaWiredKeymapBlob,
    ) -> oneshot::Receiver<Result<(), WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(SetAulaWiredKeymap { route, blob, reply });
        }
        receiver
    }

    /// Request a fresh read of all verified F87 PRO settings.
    pub(crate) fn read_aula_settings(
        &self,
        route: DeviceRoute,
    ) -> oneshot::Receiver<Result<AulaState, WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(ReadAulaSettings { route, reply });
        }
        receiver
    }

    /// Apply/save a verified F87 PRO settings edit through the agent.
    pub(crate) fn set_aula_settings(
        &self,
        route: DeviceRoute,
        change: AulaChange,
        mode: LightingMode,
    ) -> oneshot::Receiver<Result<AulaState, WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(SetAulaSettings {
                route,
                change,
                mode,
                reply,
            });
        }
        receiver
    }

    /// Apply a complete per-key RGB table through the agent.
    pub(crate) fn set_aula_per_key_rgb(
        &self,
        route: DeviceRoute,
        colors: AulaPerKeyRgb,
    ) -> oneshot::Receiver<Result<AulaState, WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(SetAulaPerKeyRgb {
                route,
                colors,
                reply,
            });
        }
        receiver
    }

    /// Request the complete base+Fn keymap through the agent.
    pub(crate) fn read_aula_keymap(
        &self,
        route: DeviceRoute,
    ) -> oneshot::Receiver<Result<AulaKeymap, WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(ReadAulaKeymap { route, reply });
        }
        receiver
    }

    /// Write both layers only if they still match the snapshot loaded for editing.
    pub(crate) fn set_aula_keymap_if_unchanged(
        &self,
        route: DeviceRoute,
        expected: AulaKeymap,
        keymap: AulaKeymap,
    ) -> oneshot::Receiver<Result<AulaKeymap, WriteError>> {
        let (reply, receiver) = oneshot::channel();
        if let Err(error) = self.aula_target(&route) {
            let _ = reply.send(Err(error));
        } else {
            self.send_ipc(SetAulaKeymapIfUnchanged {
                route,
                expected,
                keymap,
                reply,
            });
        }
        receiver
    }
}
