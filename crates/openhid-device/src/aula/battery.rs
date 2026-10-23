//! F87 PRO wireless battery readback.

use openhid_core::device::{BatteryFreshness, BatteryInfo, BatteryLevel, BatteryStatus};

use super::{
    AulaBattery, AulaBatteryPowerSource, FRAME_LENGTH, READ_TIMEOUT, ReadError, read_config,
    read_reports, select_node, validated_frame,
};
use crate::backend::HidBackend;

const BATTERY_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x4a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x5d,
];

/// Read the complete F87 PRO settings and attach its optional battery reading.
///
/// Settings remain available when the independent battery command times out or
/// returns a value this build does not recognize.
pub async fn read_selected_state(
    backend: &dyn HidBackend,
    identity: Option<&str>,
) -> Result<openhid_core::hid::aula::AulaState, ReadError> {
    let node = select_node(backend, identity).await?;
    read_state(backend, &node).await
}

pub(crate) async fn read_state(
    backend: &dyn HidBackend,
    node: &crate::backend::NodeInfo,
) -> Result<openhid_core::hid::aula::AulaState, ReadError> {
    let config = read_config(backend, node).await?;
    let battery = match read_battery(backend, node).await {
        Ok(battery) => battery,
        Err(error) => {
            tracing::debug!(%error, "AULA battery read unavailable; retaining settings readback");
            None
        }
    };
    Ok(config.aula_state().with_battery(battery))
}

async fn read_battery(
    backend: &dyn HidBackend,
    node: &crate::backend::NodeInfo,
) -> Result<Option<AulaBattery>, ReadError> {
    tokio::time::timeout(READ_TIMEOUT, async {
        let mut session = backend.open_raw_session(node).await?;
        let frames = read_reports(&mut *session, &BATTERY_REQUEST, battery_frame).await?;
        let frame = frames
            .first()
            .ok_or(ReadError::InvalidResponse("missing battery report"))?;
        battery_info(frame)
    })
    .await
    .map_err(|_| ReadError::Timeout)?
}

fn battery_frame(report: &[u8]) -> Result<Option<[u8; FRAME_LENGTH]>, ReadError> {
    let Some(frame) = validated_frame(report, 0x13, 0x4a, 1)? else {
        return Ok(None);
    };
    if frame[2] != 1 || frame[3] != 0 || frame[4] != 2 {
        return Err(ReadError::InvalidResponse("invalid battery report shape"));
    }
    Ok(Some(frame))
}

fn battery_info(frame: &[u8; FRAME_LENGTH]) -> Result<Option<AulaBattery>, ReadError> {
    let (power_source, status) = match frame[6] {
        0x01 => (AulaBatteryPowerSource::Battery, BatteryStatus::Discharging),
        // The protocol distinguishes cable power from battery power but does
        // not say whether a connected device is actively charging or full.
        0x10 => (AulaBatteryPowerSource::Cable, BatteryStatus::Unknown),
        _ => return Err(ReadError::InvalidResponse("unknown battery power state")),
    };
    let percentage = match frame[5] {
        0 => None,
        percentage @ 1..=100 => Some(percentage),
        _ => return Err(ReadError::InvalidResponse("battery percentage exceeds 100")),
    };
    Ok(Some(AulaBattery {
        info: BatteryInfo {
            freshness: BatteryFreshness::Current,
            percentage,
            level: BatteryLevel::Unknown,
            status,
        },
        power_source,
    }))
}
