//! Matrix-only backups retain complete wireless replies, separate from configuration.

use super::{FRAME_LENGTH, ReadError, WirelessMatrix};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Backup {
    version: u8,
    driver: String,
    region: String,
    frames: Vec<[u8; FRAME_LENGTH]>,
}

impl From<WirelessMatrix> for Backup {
    fn from(matrix: WirelessMatrix) -> Self {
        Self {
            version: 1,
            driver: openhid_device_registry::aula::DRIVER_ID.into(),
            region: "wireless-base-matrix".into(),
            frames: matrix.frames,
        }
    }
}

impl TryFrom<Backup> for WirelessMatrix {
    type Error = ReadError;

    fn try_from(backup: Backup) -> Result<Self, Self::Error> {
        if backup.version != 1
            || backup.driver != openhid_device_registry::aula::DRIVER_ID
            || backup.region != "wireless-base-matrix"
        {
            return Err(ReadError::InvalidResponse(
                "unsupported matrix backup version, device family or region",
            ));
        }
        Self::from_frames(backup.frames)
    }
}
