//! Full-table encoder for the F87 PRO-family wireless per-key RGB reports.
//!
//! The 0x02/0x1c layout is documented by a live F75 reference implementation,
//! not yet verified with a physical F87 PRO. This module only encodes reports;
//! it performs no I/O and makes no claim that the keyboard accepted them.

use openhid_core::hid::aula::{AULA_PER_KEY_LED_COUNT, AulaPerKeyRgb, AulaState};
use std::time::Duration;
use thiserror::Error;

use crate::backend::HidBackend;

use super::{
    ConfigError, FRAME_LENGTH, ReadError, WirelessConfig, checksum, read_session, select_node,
    transaction,
};

const PLANE_COUNT: usize = 3;
const FRAGMENTS_PER_PLANE: usize = 9;
const LEDS_PER_FRAGMENT: usize = 14;
const DATA_FRAGMENT_COUNT: usize = PLANE_COUNT * FRAGMENTS_PER_PLANE;
const REPORT_COUNT: usize = DATA_FRAGMENT_COUNT + 1;
const REPORT_TIMEOUT: Duration = Duration::from_secs(1);
const _: () = assert!(REPORT_COUNT <= 256);

/// Encode a complete 126-LED table as 27 planar reports and its trailer.
///
/// Channels are sent in red, green, blue order; each report carries fourteen
/// consecutive LED indices. A full validated table is required, so omitted
/// colors can never be silently substituted with black.
#[must_use]
pub fn encode_wireless_per_key_frames(
    colors: &AulaPerKeyRgb,
) -> [[u8; FRAME_LENGTH]; REPORT_COUNT] {
    let mut reports = [[0; FRAME_LENGTH]; REPORT_COUNT];
    for plane in 0..PLANE_COUNT {
        for fragment in 0..FRAGMENTS_PER_PLANE {
            let sequence = plane * FRAGMENTS_PER_PLANE + fragment;
            let report = &mut reports[sequence];
            report[0] = 0x13;
            report[1] = 0x02;
            report[2] = 0x1c;
            report[3] = sequence.to_le_bytes()[0];
            report[4] = 0x0e;
            for offset in 0..LEDS_PER_FRAGMENT {
                let led = fragment * LEDS_PER_FRAGMENT + offset;
                let channel = colors.colors()[led].channels()[plane];
                report[5 + offset] = channel;
            }
            report[19] = checksum(report);
        }
    }

    let trailer = &mut reports[DATA_FRAGMENT_COUNT];
    trailer[0] = 0x13;
    trailer[1] = 0x02;
    trailer[2] = 0x1c;
    trailer[3] = DATA_FRAGMENT_COUNT.to_le_bytes()[0];
    trailer[4] = 0x06;
    trailer[7] = 0x5a;
    trailer[8] = 0xa5;
    trailer[19] = checksum(trailer);

    reports
}

/// Select the custom effect, write a complete table, then save it.
///
/// The sequence follows the field-tested F87-family wireless reference. The
/// F87 wireless protocol has no verified table readback: exact per-frame
/// echoes are required, then the built-in configuration is read back after
/// SAVE. A table-write failure attempts to restore and save the original
/// lighting configuration; it cannot restore the previous RGB table.
pub async fn apply_selected_per_key_rgb(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    colors: &AulaPerKeyRgb,
    cancelled: impl Fn() -> bool,
) -> Result<AulaState, PerKeyRgbError> {
    let node = select_node(backend, identity).await?;
    let mut session = tokio::time::timeout(REPORT_TIMEOUT, backend.open_raw_session(&node))
        .await
        .map_err(|_| PerKeyRgbError::Timeout)?
        .map_err(ReadError::from)?;
    let before = read_session(&mut *session).await?;
    if before.lighting().effect == 21 {
        return Err(PerKeyRgbError::CustomEffectAlreadyActive);
    }
    if cancelled() {
        return Err(PerKeyRgbError::Cancelled);
    }

    let target = custom_effect_config(&before);
    if let Err(cause) = transaction::write_verified(&mut *session, &target, &|| false, false).await
    {
        return Err(restore_after_failure(
            &mut *session,
            &before,
            PerKeyRgbError::Configuration(cause),
        )
        .await);
    }

    for (sequence, frame) in encode_wireless_per_key_frames(colors).iter().enumerate() {
        if let Err(cause) = transaction::send_and_await_echo(&mut *session, frame).await {
            let cause = PerKeyRgbError::PartialWrite {
                completed_reports: u8::try_from(sequence).unwrap_or(u8::MAX),
                failed_sequence: u8::try_from(sequence).unwrap_or(u8::MAX),
                cause: Box::new(cause),
            };
            return Err(restore_after_failure(&mut *session, &before, cause).await);
        }
    }

    let applied = match transaction::save_and_readback(&mut *session, &target).await {
        Ok(applied) => applied,
        Err(cause) => {
            return Err(restore_after_failure(
                &mut *session,
                &before,
                PerKeyRgbError::Configuration(cause),
            )
            .await);
        }
    };
    Ok(applied.aula_state())
}

async fn restore_after_failure(
    session: &mut dyn crate::backend::RawSession,
    before: &WirelessConfig,
    cause: PerKeyRgbError,
) -> PerKeyRgbError {
    match transaction::write_verified(session, before, &|| false, true).await {
        Ok(_) => PerKeyRgbError::ConfigurationRestored(Box::new(cause)),
        Err(recovery) => PerKeyRgbError::RecoveryFailed {
            cause: Box::new(cause),
            recovery: Box::new(recovery),
        },
    }
}

fn custom_effect_config(before: &WirelessConfig) -> WirelessConfig {
    let mut target = before.clone();
    target.frames[0][8] = 0x01;
    target.frames[0][14] = 0x00;
    target.frames[0][15] = 21;
    target.frames[0][17] = 0x01;
    for frame in &mut target.frames {
        frame[19] = checksum(frame);
    }
    target
}

/// Failure while applying an F87 PRO-family wireless custom RGB table.
#[derive(Debug, Error)]
pub enum PerKeyRgbError {
    /// The device could not be selected or its baseline configuration read.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// A write or read operation exceeded its protocol deadline.
    #[error("AULA per-key RGB operation timed out")]
    Timeout,
    /// Refuse to replace the table while custom mode is active.
    #[error("select a built-in AULA effect before replacing the per-key RGB table")]
    CustomEffectAlreadyActive,
    /// The request was cancelled before its first table report was sent.
    #[error("AULA per-key RGB operation cancelled before writing")]
    Cancelled,
    /// A failed report can leave an acknowledged prefix in device storage.
    #[error(
        "AULA per-key RGB stopped at report {failed_sequence} after {completed_reports} of {REPORT_COUNT} reports were acknowledged; the table may be partial: {cause}"
    )]
    PartialWrite {
        /// Color reports with exact echoes before the failed report.
        completed_reports: u8,
        /// Sequence of the report whose write or exact echo failed.
        failed_sequence: u8,
        /// Underlying write or echo failure.
        cause: Box<ConfigError>,
    },
    /// The custom configuration operation failed.
    #[error("AULA custom RGB configuration failed: {0}")]
    Configuration(ConfigError),
    /// The original lighting configuration was restored after a failed update.
    #[error(
        "AULA custom RGB update failed; original lighting configuration restored, but the RGB table may have changed: {0}"
    )]
    ConfigurationRestored(Box<Self>),
    /// The update and restoration of the original configuration both failed.
    #[error(
        "AULA custom RGB update failed ({cause}); restoring the original lighting configuration failed ({recovery})"
    )]
    RecoveryFailed {
        /// Failure during the requested update.
        cause: Box<Self>,
        /// Failure while restoring and saving the original configuration.
        recovery: Box<ConfigError>,
    },
}

const _: () = assert!(AULA_PER_KEY_LED_COUNT == FRAGMENTS_PER_PLANE * LEDS_PER_FRAGMENT);

#[cfg(test)]
mod tests {
    use super::*;
    use openhid_core::hid::aula::{AULA_PER_KEY_LED_COUNT, AulaRgb};

    #[test]
    fn per_key_wire_table_is_complete_and_planar() {
        let colors = AulaPerKeyRgb::new(
            (0..AULA_PER_KEY_LED_COUNT)
                .map(|index| {
                    let led = u8::try_from(index).unwrap();
                    AulaRgb::new(led, led.wrapping_add(1), led.wrapping_add(2))
                })
                .collect(),
        )
        .unwrap();
        let reports = encode_wireless_per_key_frames(&colors);

        assert_eq!(reports.len(), 28);
        for (index, report) in reports.iter().enumerate() {
            assert_eq!(&report[..3], &[0x13, 0x02, 0x1c]);
            assert_eq!(report[3], u8::try_from(index).unwrap());
            assert_eq!(report[19], checksum(report));
        }
        for (sequence, channel) in [(0, 0), (9, 1), (18, 2)] {
            let report = &reports[sequence];
            assert_eq!(report[4], 0x0e);
            for offset in 0..LEDS_PER_FRAGMENT {
                let led = sequence % FRAGMENTS_PER_PLANE * LEDS_PER_FRAGMENT + offset;
                assert_eq!(report[5 + offset], colors.colors()[led].channels()[channel]);
            }
        }
        assert_eq!(
            &reports[DATA_FRAGMENT_COUNT][4..9],
            &[0x06, 0, 0, 0x5a, 0xa5]
        );
    }
}
