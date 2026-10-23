//! One owner for F87 PRO staging, SAVE, verification and recovery.

use super::{AulaChange, AulaState, LightingChange, LightingMode, LightingState};
use super::{FRAME_LENGTH, ReadError, WirelessConfig, checksum, read_session, select_node};
use crate::backend::{BackendError, HidBackend, RawSession};
use openhid_core::hid::aula::AulaColorModeChange;
use std::time::Duration;
use thiserror::Error;

const TRANSACTION_TIMEOUT: Duration = Duration::from_secs(12);
// Captured from this receiver: SAVE confirms command 0x0a / subcommand 1.
const SAVE_REQUEST: [u8; FRAME_LENGTH] = [
    0x13, 0x0a, 1, 0, 4, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x29,
];

/// A validated configuration edit; both variants share recovery and persistence.
pub enum ConfigChange {
    /// Preserve all bytes except explicitly selected lighting fields.
    Lighting(LightingChange),
    /// Change the supported F87 PRO settings while preserving every other byte.
    Settings(AulaChange),
    /// Restore a validated configuration block from this device family.
    Restore(WirelessConfig),
}

/// Verified snapshots; preview success also proves exact restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigResult {
    /// Original configuration captured before any writes.
    pub before: WirelessConfig,
    /// Configuration read back after the requested change.
    pub changed: WirelessConfig,
    /// State left on the keyboard, including restoration after a preview.
    pub current: LightingState,
    /// All supported settings read back after the transaction.
    pub state: AulaState,
}

/// A failed update never silently implies that restoration succeeded.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Rejected before any writes; current mode may lack a parameter table.
    #[error("unsupported AULA configuration edit (effects 0..18, levels 0..4)")]
    InvalidSetting,
    /// Read, transport, or framing failure.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// Complete apply/restore phase exceeded its deadline.
    #[error("AULA lighting transaction timed out")]
    Timeout,
    /// A particular command did not receive a matching echo in its deadline.
    #[error(
        "AULA echo timed out for command {command:#04x}, subcommand {subcommand:#04x}, sequence {sequence}"
    )]
    EchoTimeout {
        /// Output command byte.
        command: u8,
        /// Output subcommand byte.
        subcommand: u8,
        /// Output fragment sequence.
        sequence: u8,
    },
    /// Caller cancellation, checked between operations before committing success.
    #[error("AULA lighting operation cancelled")]
    Cancelled,
    /// Echoes do not prove settings applied; full configuration must match.
    #[error("AULA configuration readback does not match the requested snapshot")]
    ReadbackMismatch,
    /// An update failed, and the original configuration was verified restored.
    #[error("AULA lighting update failed; original configuration restored: {0}")]
    Restored(Box<Self>),
    /// Configuration recovery cannot restore the separately uploaded palette.
    #[error(
        "AULA configuration was restored, but the separately uploaded color palette may remain changed: {0}"
    )]
    PaletteNotRestored(Box<Self>),
    /// Palette transmission stopped after an acknowledged prefix.
    #[error(
        "AULA color palette stopped at report {failed_sequence} after {completed_reports} of 37 reports; color data may be partial: {cause}"
    )]
    PalettePartial {
        /// Palette reports with exact echoes before the failed report.
        completed_reports: u8,
        /// Sequence of the report whose write or exact echo failed.
        failed_sequence: u8,
        /// Underlying write, echo, or cancellation failure.
        cause: Box<Self>,
    },
    /// The edit was verified, but preview restoration could not be verified.
    #[error("AULA preview applied; original configuration restoration could not be verified: {0}")]
    PreviewRestoreFailed(Box<Self>),
    /// Both the update and recovery failed; manual recovery is required.
    #[error(
        "AULA lighting update failed ({cause}); restoration could not be verified ({recovery})"
    )]
    RecoveryFailed {
        /// Original update failure.
        cause: Box<Self>,
        /// Failure while restoring the original configuration.
        recovery: Box<Self>,
    },
}

/// Apply a configuration edit on one uniquely selected wireless keyboard.
///
/// Host callers must run this future in an owned lighting job: cancellation may
/// signal `cancelled`, but must not destroy the future before recovery completes.
/// One session owns the snapshot, writes, verification and compensation. No
/// default configuration is substituted. Explicit color-mode edits upload the
/// family reference's palette template; the palette has no readback. SAVE is
/// sent only in Save mode, after config echoes, config readback and palette ACKs.
pub async fn update_config(
    backend: &dyn HidBackend,
    identity: Option<&str>,
    change: ConfigChange,
    mode: LightingMode,
    cancelled: impl Fn() -> bool,
) -> Result<ConfigResult, ConfigError> {
    let node = select_node(backend, identity).await?;
    let mut session = tokio::time::timeout(TRANSACTION_TIMEOUT, backend.open_raw_session(&node))
        .await
        .map_err(|_| ConfigError::Timeout)?
        .map_err(ReadError::from)?;
    let before = read_session(&mut *session).await?;
    let color_mode = match &change {
        ConfigChange::Lighting(change) => change.color_mode(),
        ConfigChange::Settings(change) => change.lighting().and_then(LightingChange::color_mode),
        ConfigChange::Restore(_) => None,
    };
    if color_mode.is_some() && mode == LightingMode::Preview {
        return Err(ConfigError::InvalidSetting);
    }
    let target = match change {
        ConfigChange::Lighting(change) => before.edited(change)?,
        ConfigChange::Settings(change) => before.edited_settings(change)?,
        ConfigChange::Restore(config) => config,
    };
    if cancelled() {
        return Err(ConfigError::Cancelled);
    }
    if before == target && mode != LightingMode::Save && color_mode.is_none() {
        return Ok(ConfigResult {
            current: before.lighting(),
            state: before.aula_state(),
            changed: before.clone(),
            before,
        });
    }
    let save = mode == LightingMode::Save;
    let update = if let Some(color_mode) = color_mode {
        write_with_palette(&mut *session, &target, color_mode, &cancelled, save).await
    } else {
        write_verified(&mut *session, &target, &cancelled, save).await
    };
    match update {
        Ok(changed) if mode != LightingMode::Preview && !cancelled() => Ok(ConfigResult {
            current: changed.lighting(),
            state: changed.aula_state(),
            before,
            changed,
        }),
        result => {
            // Recovery deliberately ignores caller cancellation, and retains the
            // same session/host lock until every in-flight write has completed.
            let recovery = write_verified(&mut *session, &before, &|| false, save).await;
            match (result, recovery) {
                (Ok(changed), Ok(restored)) if !cancelled() => Ok(ConfigResult {
                    current: restored.lighting(),
                    state: restored.aula_state(),
                    before,
                    changed,
                }),
                (result, Ok(_)) => {
                    let cause = Box::new(result.err().unwrap_or(ConfigError::Cancelled));
                    if color_mode.is_some() {
                        Err(ConfigError::PaletteNotRestored(cause))
                    } else {
                        Err(ConfigError::Restored(cause))
                    }
                }
                (Ok(_), Err(recovery)) if !cancelled() => {
                    Err(ConfigError::PreviewRestoreFailed(Box::new(recovery)))
                }
                (result, Err(recovery)) => Err(ConfigError::RecoveryFailed {
                    cause: Box::new(result.err().unwrap_or(ConfigError::Cancelled)),
                    recovery: Box::new(recovery),
                }),
            }
        }
    }
}

async fn write_with_palette(
    session: &mut dyn RawSession,
    config: &WirelessConfig,
    color_mode: AulaColorModeChange,
    cancelled: &impl Fn() -> bool,
    save: bool,
) -> Result<WirelessConfig, ConfigError> {
    let staged = write_verified(session, config, cancelled, false).await?;
    let color = match color_mode {
        AulaColorModeChange::SingleColor(color) => Some(color),
        AulaColorModeChange::Colorful => None,
    };
    super::palette::write_palette(session, color, cancelled).await?;
    if save {
        save_and_readback(session, config).await
    } else {
        Ok(staged)
    }
}

pub(super) async fn write_verified(
    session: &mut dyn RawSession,
    config: &WirelessConfig,
    cancelled: &impl Fn() -> bool,
    save: bool,
) -> Result<WirelessConfig, ConfigError> {
    tokio::time::timeout(TRANSACTION_TIMEOUT, async {
        for original in &config.frames {
            if cancelled() {
                return Err(ConfigError::Cancelled);
            }
            let mut frame = *original;
            frame[1] = 0x04;
            frame[19] = checksum(&frame);
            send_and_await_echo(session, &frame).await?;
            tracing::debug!(sequence = frame[3], "AULA configuration fragment echoed");
        }
        let mut actual = readback(session, config).await?;
        if save {
            if cancelled() {
                return Err(ConfigError::Cancelled);
            }
            actual = save_and_readback(session, config).await?;
        }
        Ok(actual)
    })
    .await
    .map_err(|_| ConfigError::Timeout)?
}

/// SAVE a staged snapshot and verify the complete device configuration again.
pub(super) async fn save_and_readback(
    session: &mut dyn RawSession,
    expected: &WirelessConfig,
) -> Result<WirelessConfig, ConfigError> {
    tokio::time::timeout(TRANSACTION_TIMEOUT, async {
        send_and_await_echo(session, &SAVE_REQUEST).await?;
        tracing::debug!("AULA save echoed");
        readback(session, expected).await
    })
    .await
    .map_err(|_| ConfigError::Timeout)?
}

pub(super) async fn readback(
    session: &mut dyn RawSession,
    expected: &WirelessConfig,
) -> Result<WirelessConfig, ConfigError> {
    // The shared reader retries idempotent queries on response timeout; never
    // repeat settings writes on a missing ACK.
    tracing::debug!("AULA reading back configuration");
    let actual = read_session(session).await?;
    if &actual != expected {
        return Err(ConfigError::ReadbackMismatch);
    }
    tracing::debug!("AULA full configuration readback matched");
    Ok(actual)
}

pub(super) async fn send_and_await_echo(
    session: &mut dyn RawSession,
    expected: &[u8; FRAME_LENGTH],
) -> Result<(), ConfigError> {
    // SAVE may include a flash commit; do not apply a sub-second fragment
    // deadline to it. Both still fit under the phase's overall deadline.
    let echo_budget = if expected[1] == SAVE_REQUEST[1] {
        Duration::from_secs(3)
    } else {
        Duration::from_secs(1)
    };
    let deadline = tokio::time::Instant::now() + echo_budget;
    tokio::time::timeout_at(deadline, async {
        let mut buffer = [0; 64];
        let length = session
            .exchange_output_report(expected, &mut buffer)
            .await
            .map_err(ReadError::from)?;
        let report = buffer.get(..length);
        tracing::debug!(
            length,
            header = ?report.and_then(|bytes| bytes.get(..4)),
            expected_header = ?&expected[..4],
            "AULA echo candidate",
        );
        if report == Some(expected.as_slice()) {
            return Ok(());
        }
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(echo_timeout(expected));
            }
            let length = session
                .read_input_report(&mut buffer)
                .await
                .map_err(ReadError::from)?;
            if length == 0 {
                return Err(ReadError::Backend(BackendError::Disconnected).into());
            }
            // Exact correlation: command/subcommand/sequence, all payload
            // bytes AND checksum. Another packet of the same command is not ACK.
            let report = buffer.get(..length);
            tracing::debug!(
                length,
                header = ?report.and_then(|bytes| bytes.get(..4)),
                expected_header = ?&expected[..4],
                "AULA echo candidate",
            );
            if report == Some(expected.as_slice()) {
                return Ok(());
            }
        }
    })
    .await
    .map_err(|_| echo_timeout(expected))?
}

fn echo_timeout(expected: &[u8; FRAME_LENGTH]) -> ConfigError {
    ConfigError::EchoTimeout {
        command: expected[1],
        subcommand: expected[2],
        sequence: expected[3],
    }
}
