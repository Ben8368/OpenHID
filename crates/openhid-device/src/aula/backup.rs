//! Versioned configuration-only backups; no serial, USB path or host identity.

use super::{
    FRAGMENT_COUNT, FRAME_LENGTH, ReadError, WirelessConfig, config_frame, validate_ordered_frames,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Backup {
    version: u8,
    driver: String,
    frames: [[u8; FRAME_LENGTH]; FRAGMENT_COUNT],
}

impl From<WirelessConfig> for Backup {
    fn from(config: WirelessConfig) -> Self {
        Self {
            version: 1,
            driver: openhid_device_registry::aula::DRIVER_ID.into(),
            frames: config.frames,
        }
    }
}

impl TryFrom<Backup> for WirelessConfig {
    type Error = ReadError;
    fn try_from(backup: Backup) -> Result<Self, Self::Error> {
        if backup.version != 1 || backup.driver != openhid_device_registry::aula::DRIVER_ID {
            return Err(ReadError::InvalidResponse(
                "unsupported backup version or device family",
            ));
        }
        Self::from_frames(backup.frames)
    }
}

impl WirelessConfig {
    /// Validate complete, canonical READ frames before accepting a saved configuration.
    /// Unknown payload bytes are retained, including custom effect identifiers.
    pub fn from_frames(frames: [[u8; FRAME_LENGTH]; FRAGMENT_COUNT]) -> Result<Self, ReadError> {
        validate_ordered_frames(&frames, config_frame)?;
        Ok(Self { frames })
    }
}
