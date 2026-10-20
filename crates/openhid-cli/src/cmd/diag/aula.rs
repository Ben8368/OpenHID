//! AULA wireless configuration and recoverable built-in lighting diagnostics.

use anyhow::{Context as _, Result};
use clap::Args;
use openhid_hid::aula::{AulaChange, LightingChange, LightingMode, SleepTimeout};
use std::{
    io::Write as _,
    path::{Path, PathBuf},
};

#[derive(Debug, Args)]
pub struct AulaArgs {
    /// Exact raw-route identity from `openhid list`, when several keyboards exist.
    #[arg(long)]
    identity: Option<String>,
    /// Experimental read-only raw wireless key matrix; no remapping or SAVE.
    #[arg(long, conflicts_with_all = ["restore", "effect", "brightness", "speed", "save", "preview", "sleep"])]
    matrix: bool,
    /// Export configuration, or the raw base matrix with --matrix, to versioned JSON.
    #[arg(long, conflicts_with_all = ["restore", "effect", "brightness", "speed", "save", "preview", "sleep"])]
    backup: Option<PathBuf>,
    /// Restore a validated configuration backup; requires --save or --preview.
    #[arg(long, conflicts_with_all = ["effect", "brightness", "speed", "sleep"])]
    restore: Option<PathBuf>,
    /// Native built-in effect ID (0 = off; 1..18 = OEM built-ins).
    #[arg(long, value_parser = parse_effect)]
    effect: Option<u8>,
    /// Brightness level 0..4 for the selected/current built-in effect.
    #[arg(long, value_parser = parse_level)]
    brightness: Option<u8>,
    /// Speed level 0..4 for the selected/current built-in effect.
    #[arg(long, value_parser = parse_level)]
    speed: Option<u8>,
    /// Wireless idle timeout in seconds: 60, 90, 120, 180, 240, 300, 600, 900 or 1200.
    #[arg(long, value_parser = parse_sleep)]
    sleep: Option<u16>,
    /// Commit verified configuration to the device with an explicit SAVE.
    #[arg(long, conflicts_with = "preview")]
    save: bool,
    /// Stage/verify the edit, then restore/verify the original without SAVE.
    #[arg(long)]
    preview: bool,
}

fn parse_effect(input: &str) -> Result<u8, String> {
    let value = input.parse::<u8>().map_err(|error| error.to_string())?;
    LightingChange::new(Some(value), None, None)
        .map(|_| value)
        .map_err(|error| error.to_string())
}

fn parse_sleep(input: &str) -> Result<u16, String> {
    let value = input.parse::<u16>().map_err(|error| error.to_string())?;
    SleepTimeout::from_seconds(value)
        .map(SleepTimeout::seconds)
        .ok_or_else(|| {
            format!(
                "choose a timeout in seconds: {}",
                SleepTimeout::options()
                    .iter()
                    .map(|value| value.seconds().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn parse_level(input: &str) -> Result<u8, String> {
    let value = input.parse::<u8>().map_err(|error| error.to_string())?;
    LightingChange::new(None, Some(value), None)
        .map(|_| value)
        .map_err(|error| error.to_string())
}

pub async fn run(args: AulaArgs) -> Result<()> {
    if args.matrix {
        let matrix = openhid_hid::read_aula_matrix(args.identity.as_deref()).await?;
        if let Some(path) = args.backup.as_deref() {
            write_backup(path, &matrix)?;
            println!(
                "Wireless base matrix backed up to {} ({} entries; no configuration, Fn layer, macros or RGB tables).",
                path.display(),
                matrix.entries().len()
            );
            return Ok(());
        }
        println!(
            "Experimental wireless matrix: {} raw entries (key/function encodings not yet verified).",
            matrix.entries().len()
        );
        for (index, entry) in matrix.entries().enumerate() {
            println!(
                "{index:03}: {:02x} {:02x} {:02x} {:02x}",
                entry[0], entry[1], entry[2], entry[3]
            );
        }
        return Ok(());
    }
    if let Some(path) = args.restore.as_deref() {
        return restore_backup(&args, path).await;
    }
    let editing = args.effect.is_some() || args.brightness.is_some() || args.speed.is_some();
    if editing || args.sleep.is_some() {
        let lighting = editing
            .then(|| LightingChange::new(args.effect, args.brightness, args.speed))
            .transpose()?;
        let sleep = args.sleep.and_then(SleepTimeout::from_seconds);
        let settings = AulaChange::new(lighting, sleep)?;
        let mode = if args.preview {
            LightingMode::Preview
        } else if args.save {
            LightingMode::Save
        } else {
            LightingMode::Apply
        };
        openhid_hid::start_aula_settings(args.identity.as_deref(), settings, mode)
            .await?
            .finish()
            .await?;
        if args.preview {
            println!(
                "AULA settings edit verified; original full configuration restored and verified."
            );
        } else if args.save {
            println!("AULA settings edit saved and verified by full configuration readback.");
        } else {
            println!("AULA settings edit staged and verified (no SAVE command).");
        }
        return Ok(());
    }
    anyhow::ensure!(
        !args.preview && !args.save,
        "--preview/--save requires an AULA settings edit"
    );
    let config = openhid_hid::read_aula_config(args.identity.as_deref()).await?;
    if let Some(path) = args.backup {
        write_backup(&path, &config)?;
        println!(
            "Configuration block backed up to {} (10 validated frames; keymap, macros and RGB tables are separate).",
            path.display()
        );
        return Ok(());
    }
    let state = config.lighting();
    println!(
        "AULA F87 PRO wireless: effect={}, brightness={:?}, speed={:?}, sleep={:?}s",
        state.effect,
        state.brightness,
        state.speed,
        config.aula_state().sleep_timeout.map(SleepTimeout::seconds)
    );
    println!("10 validated configuration fragments:");
    for (sequence, frame) in config.frames().iter().enumerate() {
        print!("{sequence:02}:");
        for byte in frame {
            print!(" {byte:02x}");
        }
        println!();
    }
    Ok(())
}

fn write_backup(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let encoded = serde_json::to_vec_pretty(value)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "create backup {} (existing files are preserved)",
                path.display()
            )
        })?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    Ok(())
}

async fn restore_backup(args: &AulaArgs, path: &Path) -> Result<()> {
    anyhow::ensure!(
        args.save || args.preview,
        "--restore requires --save or --preview"
    );
    let encoded = std::fs::read(path)
        .with_context(|| format!("read configuration backup {}", path.display()))?;
    let config = serde_json::from_slice::<openhid_hid::aula::WirelessConfig>(&encoded)
        .context("invalid F87 PRO configuration backup")?;
    let mode = if args.preview {
        LightingMode::Preview
    } else {
        LightingMode::Save
    };
    openhid_hid::start_aula_restore(args.identity.as_deref(), config, mode)
        .await?
        .finish()
        .await?;
    println!(
        "{}",
        if args.preview {
            "Backup staged and verified; original device configuration restored and verified."
        } else {
            "Configuration backup restored, saved and verified."
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        aula: AulaArgs,
    }

    #[test]
    fn lighting_parser_rejects_invalid_native_levels_and_ambiguous_commit_mode() {
        for args in [
            vec!["test", "--effect", "19"],
            vec!["test", "--matrix", "--save"],
            vec!["test", "--matrix", "--sleep", "600"],
            vec!["test", "--brightness", "5"],
            vec!["test", "--speed", "255"],
            vec!["test", "--sleep", "301"],
            vec!["test", "--brightness", "2", "--save", "--preview"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        let args = Cli::parse_from(["test", "--effect", "1", "--brightness", "4", "--save"]).aula;
        assert_eq!(args.effect, Some(1));
        assert_eq!(args.brightness, Some(4));
        assert!(args.save);
        assert!(!args.preview);
        assert_eq!(
            Cli::parse_from(["test", "--sleep", "600"]).aula.sleep,
            Some(600)
        );
    }
    #[test]
    fn configuration_backup_conflicts_with_edits_and_restore_accepts_explicit_commit_modes() {
        for args in [
            vec!["test", "--backup", "config.json", "--sleep", "600"],
            vec!["test", "--backup", "config.json", "--save"],
            vec!["test", "--backup", "config.json", "--restore", "other.json"],
            vec![
                "test",
                "--restore",
                "config.json",
                "--brightness",
                "2",
                "--save",
            ],
            vec![
                "test",
                "--restore",
                "config.json",
                "--sleep",
                "600",
                "--save",
            ],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        for mode in ["--save", "--preview"] {
            let args = Cli::parse_from(["test", "--restore", "config.json", mode]).aula;
            assert_eq!(
                args.restore.as_deref(),
                Some(std::path::Path::new("config.json"))
            );
        }
    }

    #[test]
    fn matrix_backup_parser_selects_a_distinct_read_only_region() {
        let args = Cli::parse_from(["test", "--matrix", "--backup", "matrix.json"]).aula;
        assert!(args.matrix);
        assert_eq!(args.backup.as_deref(), Some(Path::new("matrix.json")));
        for flag in ["--save", "--preview", "--restore"] {
            let mut invocation = vec!["test", "--matrix", "--backup", "matrix.json", flag];
            if flag == "--restore" {
                invocation.push("config.json");
            }
            assert!(Cli::try_parse_from(invocation).is_err());
        }
    }

    #[tokio::test]
    async fn matrix_backup_cannot_be_restored_as_configuration_before_hardware_is_opened() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("matrix.json");
        // Region metadata alone already proves this is not a configuration backup.
        write_backup(
            &path,
            &serde_json::json!({
                "version": 1,
                "driver": "aula-f87-pro-wireless",
                "region": "wireless-base-matrix",
                "frames": []
            }),
        )
        .unwrap();
        let args = Cli::parse_from(["test", "--restore", path.to_str().unwrap(), "--save"]).aula;
        let error = run(args).await.unwrap_err();
        assert_eq!(error.to_string(), "invalid F87 PRO configuration backup");
    }

    #[tokio::test]
    async fn restore_without_a_commit_mode_fails_before_reading_a_file_or_hardware() {
        let args = Cli::parse_from(["test", "--restore", "missing.json"]).aula;
        let error = run(args).await.unwrap_err();
        assert_eq!(error.to_string(), "--restore requires --save or --preview");
    }
}
