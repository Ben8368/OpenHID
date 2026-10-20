//! F87 PRO commands use device-owned memory, independently of HID++ RGB config.

use openhid_core::hid::aula::{AulaChange, AulaState, LightingMode, route_identity};
use openhid_core::hid::{DeviceRoute, WriteError};
use tokio::sync::oneshot;

use super::AppState;
use crate::services::ipc::{ReadAulaSettings, SetAulaSettings};

impl AppState {
    fn aula_target(&self, route: &DeviceRoute) -> Result<(), WriteError> {
        if route_identity(route).is_none()
            || !self
                .current_record()
                .is_some_and(|record| record.online && record.route.as_ref() == Some(route))
        {
            return Err(WriteError::DeviceNotFound);
        }
        Ok(())
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
}
