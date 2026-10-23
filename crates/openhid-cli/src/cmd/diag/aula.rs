//! AULA wireless configuration and recoverable built-in lighting diagnostics.

use anyhow::{Context as _, Result};
use clap::Args;
use openhid_core::hid::aula::{
    AULA_F87_KEY_POSITIONS, AulaColorModeChange, AulaDebounce, AulaKeymap, AulaKeymapAction,
    AulaKeymapProfile, AulaOemBaseLayer, AulaOemFnLayer, AulaPerKeyRgbProfile, AulaRgb,
    AulaSideLightEffect,
};
use openhid_hid::aula::{
    AulaChange, LightingChange, LightingMode, SleepTimeout,
    plan_keymap_write as prepare_keymap_write_plan,
};
use std::{
    io::Write as _,
    path::{Path, PathBuf},
};

#[derive(Debug, Args)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Clap models independent presence-only diagnostic flags as bool fields"
)]
pub struct AulaArgs {
    /// Exact raw-route identity from `openhid list`, when several keyboards exist.
    #[arg(long)]
    identity: Option<String>,
    /// Read the six-byte OEM wireless model discriminator without changing settings.
    #[arg(long, conflicts_with_all = [
        "matrix", "oem_matrix_probe", "oem_keymap_probe", "macro_prefix", "keymap", "import_oem_profile",
        "export_keymap", "backup", "restore", "restore_keymap", "apply",
        "plan_keymap_write", "rgb_profile", "effect", "brightness", "speed", "color",
        "colorful", "sleep", "debounce", "side_light_effect", "save", "preview"
    ])]
    psd: bool,
    /// Query wired 258A:010C collections for an F87 Pro using Feature Report 0x06/0x82; read-only.
    #[arg(long, conflicts_with_all = [
        "psd", "matrix", "oem_matrix_probe", "oem_keymap_probe", "macro_prefix", "keymap",
        "import_oem_profile", "export_keymap", "backup", "restore", "restore_keymap", "apply",
        "plan_keymap_write", "rgb_profile", "effect", "brightness", "speed", "color",
        "colorful", "sleep", "debounce", "side_light_effect", "save", "preview"
    ])]
    wired_model_probe: bool,
    /// Read the raw wireless base matrix without changing it.
    #[arg(long, conflicts_with_all = ["restore", "restore_keymap", "keymap", "apply", "effect", "brightness", "speed", "save", "preview", "sleep", "debounce", "side_light_effect", "rgb_profile", "color", "colorful"])]
    matrix: bool,
    /// Read the base matrix using the OEM's unadvertised 0x15 carrier (Linux/Windows raw-HID diagnostic).
    #[arg(
        long,
        conflicts_with_all = [
            "matrix", "macro_prefix", "keymap", "import_oem_profile", "export_keymap",
            "restore", "restore_keymap", "apply", "plan_keymap_write", "rgb_profile",
            "effect", "brightness", "speed", "color", "colorful", "sleep", "debounce", "side_light_effect",
            "save", "preview"
        ]
    )]
    oem_matrix_probe: bool,
    /// Read both base and Fn layers through the OEM's unadvertised 0x15 carrier (Linux/Windows raw-HID diagnostic).
    #[arg(
        long,
        conflicts_with_all = [
            "matrix", "oem_matrix_probe", "macro_prefix", "keymap", "import_oem_profile",
            "export_keymap", "restore", "restore_keymap", "apply", "plan_keymap_write",
            "rgb_profile", "effect", "brightness", "speed", "color", "colorful", "sleep",
            "debounce", "side_light_effect", "save", "preview"
        ]
    )]
    oem_keymap_probe: bool,
    /// Read the experimental 512-byte wireless macro prefix through the running Agent; content is not displayed.
    #[arg(long, conflicts_with_all = ["matrix", "oem_matrix_probe", "oem_keymap_probe", "keymap", "import_oem_profile", "export_keymap", "backup", "restore", "restore_keymap", "apply", "plan_keymap_write", "rgb_profile", "effect", "brightness", "speed", "color", "colorful", "sleep", "debounce", "side_light_effect", "save", "preview"])]
    macro_prefix: bool,
    /// Save the incomplete raw prefix to a new file; requires --macro-prefix.
    #[arg(long, requires = "macro_prefix")]
    macro_prefix_output: Option<PathBuf>,
    /// Repeat the wireless macro query to the generic OEM aggregate encoder's 0x2800-byte ceiling.
    #[arg(
        long,
        conflicts_with_all = [
            "psd", "wired_model_probe", "matrix", "oem_matrix_probe", "oem_keymap_probe",
            "macro_prefix", "keymap", "import_oem_profile", "export_keymap", "backup",
            "restore", "restore_keymap", "apply", "plan_keymap_write", "rgb_profile",
            "effect", "brightness", "speed", "color", "colorful", "sleep", "debounce",
            "side_light_effect", "save", "preview"
        ]
    )]
    macro_table_candidate: bool,
    /// Save the incomplete candidate bytes to a new file; requires --macro-table-candidate.
    #[arg(long, requires = "macro_table_candidate")]
    macro_table_candidate_output: Option<PathBuf>,
    /// Experimentally read both raw keymap layers; with --backup, export the response.
    #[arg(long, conflicts_with_all = ["matrix", "oem_keymap_probe", "restore", "restore_keymap", "apply", "effect", "brightness", "speed", "save", "preview", "sleep", "debounce", "side_light_effect", "rgb_profile", "color", "colorful"])]
    keymap: bool,
    /// Import base and Fn assignments from an installed F87 PRO profile.dct file.
    #[arg(
        long,
        requires = "export_keymap",
        conflicts_with_all = ["matrix", "keymap", "backup", "restore", "restore_keymap", "apply", "rgb_profile", "effect", "brightness", "speed", "color", "colorful", "sleep", "debounce", "side_light_effect", "save", "preview"]
    )]
    import_oem_profile: Option<PathBuf>,
    /// Write the imported base+Fn keymap to a new OpenHID JSON profile file.
    #[arg(
        long,
        requires = "import_oem_profile",
        conflicts_with_all = ["matrix", "keymap", "backup", "restore", "restore_keymap", "apply", "rgb_profile", "effect", "brightness", "speed", "color", "colorful", "sleep", "debounce", "side_light_effect", "save", "preview"]
    )]
    export_keymap: Option<PathBuf>,
    /// Export configuration, --matrix base data, or --keymap layers to versioned JSON.
    #[arg(long, conflicts_with_all = ["restore", "restore_keymap", "apply", "effect", "brightness", "speed", "save", "preview", "sleep", "debounce", "side_light_effect", "rgb_profile", "color", "colorful"])]
    backup: Option<PathBuf>,
    /// Restore a validated configuration backup; requires --save or --preview.
    #[arg(long, conflicts_with_all = ["restore_keymap", "apply", "effect", "brightness", "speed", "sleep", "debounce", "side_light_effect", "rgb_profile", "color", "colorful"])]
    restore: Option<PathBuf>,
    /// Restore a base+Fn backup (currently refused until F87 wireless writes are verified).
    #[arg(long, requires = "apply", conflicts_with_all = ["matrix", "keymap", "backup", "restore", "effect", "brightness", "speed", "save", "preview", "sleep", "debounce", "side_light_effect", "rgb_profile", "color", "colorful"])]
    restore_keymap: Option<PathBuf>,
    /// Confirm the persistent keymap write requested by --restore-keymap.
    #[arg(long, requires = "restore_keymap", conflicts_with_all = ["matrix", "keymap", "backup", "restore", "effect", "brightness", "speed", "save", "preview", "sleep", "debounce", "side_light_effect", "rgb_profile", "color", "colorful"])]
    apply: bool,
    /// Print an OEM-shaped write-frame plan for a local keymap profile; no device I/O.
    #[arg(
        long,
        conflicts_with_all = [
            "matrix", "keymap", "import_oem_profile", "export_keymap",
            "backup", "restore", "restore_keymap", "apply", "rgb_profile", "effect",
            "brightness", "speed", "color", "colorful", "sleep", "debounce", "side_light_effect", "save",
            "preview"
        ]
    )]
    plan_keymap_write: Option<PathBuf>,
    /// Apply an experimental 126-LED RGB profile; requires --save.
    #[arg(
        long,
        requires = "save",
        conflicts_with_all = ["matrix", "backup", "restore", "effect", "brightness", "speed", "sleep", "debounce", "side_light_effect", "preview", "color", "colorful"]
    )]
    rgb_profile: Option<PathBuf>,
    /// Native built-in effect ID (0 = off; 1..18 = OEM built-ins).
    #[arg(long, value_parser = parse_effect, conflicts_with = "rgb_profile")]
    effect: Option<u8>,
    /// Brightness level 0..4 for the selected/current built-in effect.
    #[arg(long, value_parser = parse_level, conflicts_with = "rgb_profile")]
    brightness: Option<u8>,
    /// Speed level 0..4 for the selected/current built-in effect.
    #[arg(long, value_parser = parse_level, conflicts_with = "rgb_profile")]
    speed: Option<u8>,
    /// Set a custom RGB color for the selected effect (for example #FF0080).
    #[arg(long, value_parser = parse_rgb, requires = "effect", conflicts_with_all = ["rgb_profile", "colorful", "preview"])]
    color: Option<AulaRgb>,
    /// Use the selected effect's colorful mode; requires --effect.
    #[arg(long, action = clap::ArgAction::Count, requires = "effect", conflicts_with_all = ["rgb_profile", "color", "preview"])]
    colorful: u8,
    /// Wireless idle timeout in seconds: 60, 90, 120, 180, 240, 300, 600, 900 or 1200.
    #[arg(long, value_parser = parse_sleep, conflicts_with = "rgb_profile")]
    sleep: Option<u16>,
    /// Wireless key debounce interval in milliseconds: 1, 2, 3, 4 or 5.
    #[arg(long, value_parser = parse_debounce, conflicts_with = "rgb_profile")]
    debounce: Option<u8>,
    /// Side-light effect: 0 = off, 1 = rainbow wave, 2 = rainbow together, 3 = solid, 4 = breathing.
    #[arg(long, value_parser = parse_side_light_effect, conflicts_with = "rgb_profile")]
    side_light_effect: Option<AulaSideLightEffect>,
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

fn parse_debounce(input: &str) -> Result<u8, String> {
    let value = input.parse::<u8>().map_err(|error| error.to_string())?;
    AulaDebounce::from_milliseconds(value)
        .map(AulaDebounce::milliseconds)
        .ok_or_else(|| "choose a debounce interval from 1 to 5 ms".to_owned())
}

fn parse_side_light_effect(input: &str) -> Result<AulaSideLightEffect, String> {
    let value = input.parse::<u8>().map_err(|error| error.to_string())?;
    AulaSideLightEffect::from_wire(value)
        .ok_or_else(|| "choose a side-light effect from 0 to 4".into())
}

fn parse_level(input: &str) -> Result<u8, String> {
    let value = input.parse::<u8>().map_err(|error| error.to_string())?;
    LightingChange::new(None, Some(value), None)
        .map(|_| value)
        .map_err(|error| error.to_string())
}

fn parse_rgb(input: &str) -> Result<AulaRgb, String> {
    let digits = input.strip_prefix('#').unwrap_or(input);
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("enter an RGB color as #RRGGBB".into());
    }
    let channel = |offset| {
        u8::from_str_radix(&digits[offset..offset + 2], 16).map_err(|error| error.to_string())
    };
    Ok(AulaRgb::new(channel(0)?, channel(2)?, channel(4)?))
}

pub async fn run(args: AulaArgs) -> Result<()> {
    if run_special_modes(&args).await? {
        return Ok(());
    }
    let editing = args.effect.is_some()
        || args.brightness.is_some()
        || args.speed.is_some()
        || args.color.is_some()
        || args.colorful > 0;
    if editing
        || args.sleep.is_some()
        || args.debounce.is_some()
        || args.side_light_effect.is_some()
    {
        return apply_settings(&args, editing).await;
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

async fn run_special_modes(args: &AulaArgs) -> Result<bool> {
    if let Some(profile_path) = args.import_oem_profile.as_deref() {
        let export_path = args
            .export_keymap
            .as_deref()
            .context("--import-oem-profile requires --export-keymap")?;
        import_oem_profile(profile_path, export_path)?;
        return Ok(true);
    }
    if let Some(path) = args.restore_keymap.as_deref() {
        restore_keymap(path, args.identity.as_deref()).await?;
        return Ok(true);
    }
    if let Some(path) = args.plan_keymap_write.as_deref() {
        plan_keymap_write(path)?;
        return Ok(true);
    }
    if args.psd {
        let psd = openhid_hid::read_aula_psd(args.identity.as_deref()).await?;
        println!("AULA F87 PRO wireless OEM model discriminator: {psd:02x?}");
        return Ok(true);
    }
    if args.wired_model_probe {
        let probe = openhid_hid::read_aula_wired_model_probe(args.identity.as_deref()).await?;
        println!(
            "AULA wired F87 Pro collection (read-only): identity={} response={:02x?}",
            probe.identity(),
            probe.response(),
        );
        return Ok(true);
    }
    if args.keymap {
        run_keymap(args).await?;
        return Ok(true);
    }
    if args.oem_keymap_probe {
        run_oem_keymap_probe(args).await?;
        return Ok(true);
    }
    if args.macro_prefix {
        run_macro_prefix(args).await?;
        return Ok(true);
    }
    if args.macro_table_candidate {
        run_macro_table_candidate(args).await?;
        return Ok(true);
    }
    if args.oem_matrix_probe {
        run_oem_matrix_probe(args).await?;
        return Ok(true);
    }
    if let Some(path) = args.rgb_profile.as_deref() {
        apply_rgb_profile(path, args.identity.as_deref()).await?;
        return Ok(true);
    }
    if let Some(path) = args.restore.as_deref() {
        restore_backup(args, path).await?;
        return Ok(true);
    }
    if args.matrix {
        let matrix = openhid_hid::read_aula_matrix(args.identity.as_deref()).await?;
        if let Some(path) = args.backup.as_deref() {
            write_backup(path, &matrix)?;
            println!(
                "Wireless base matrix backed up to {} ({} entries; no configuration, Fn layer, macros or RGB tables).",
                path.display(),
                matrix.entries().len()
            );
            return Ok(true);
        }
        println!(
            "Experimental wireless base matrix: {} raw entries; physical key names use the verified OEM slot correlation.",
            matrix.entries().len()
        );
        for (index, entry) in matrix.entries().enumerate() {
            let label = AULA_F87_KEY_POSITIONS
                .iter()
                .find(|key| usize::from(key.matrix_slot()) == index)
                .map_or("unmapped", |key| key.label());
            println!(
                "{index:03} {label:<10} {:02x} {:02x} {:02x} {:02x}",
                entry[0], entry[1], entry[2], entry[3]
            );
        }
        return Ok(true);
    }
    Ok(false)
}

async fn run_macro_prefix(args: &AulaArgs) -> Result<()> {
    let client = crate::agent::connect()
        .await
        .context("connect to the running OpenHID Agent for the serialized AULA macro read")?;
    let route = selected_aula_macro_route(&client, args).await?;
    let bytes = openhid_ipc::client::aula::read_macro_prefix(&client, route)
        .await
        .context("the running Agent disconnected during the AULA macro-prefix read")?
        .map_err(|error| {
            anyhow::anyhow!("the Agent could not read the AULA macro prefix: {error}")
        })?;
    if let Some(path) = args.macro_prefix_output.as_deref() {
        write_raw_dump(path, &bytes)?;
        println!(
            "Experimental wireless macro prefix saved to {} ({} bytes; incomplete and not a restorable backup).",
            path.display(),
            bytes.len()
        );
        return Ok(());
    }
    println!(
        "Experimental wireless macro query returned {} bytes. This is only an incomplete prefix; contents were not displayed.",
        bytes.len()
    );
    Ok(())
}

async fn run_macro_table_candidate(args: &AulaArgs) -> Result<()> {
    let client = crate::agent::connect()
        .await
        .context("connect to the running OpenHID Agent for the serialized AULA macro read")?;
    let route = selected_aula_macro_route(&client, args).await?;
    let reads = openhid_ipc::client::aula::read_macro_table_candidate(&client, route)
        .await
        .context("the running Agent disconnected during the AULA macro-table candidate read")?
        .map_err(|error| {
            anyhow::anyhow!("the Agent could not read the AULA macro-table candidate: {error}")
        })?;
    let bytes = reads.iter().flatten().copied().collect::<Vec<_>>();
    let unique_reads = reads
        .iter()
        .map(Vec::as_slice)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let all_zero_reads = reads
        .iter()
        .filter(|read| read.iter().all(|byte| *byte == 0))
        .count();
    if let Some(path) = args.macro_table_candidate_output.as_deref() {
        write_raw_dump(path, &bytes)?;
        println!(
            "Experimental wireless macro-table candidate saved to {} ({} bytes; {} reads, {} distinct responses, {} all-zero reads; completeness and address advancement are unverified, not a restorable backup).",
            path.display(),
            bytes.len(),
            reads.len(),
            unique_reads,
            all_zero_reads,
        );
        return Ok(());
    }
    println!(
        "Experimental wireless macro-table candidate returned {} bytes ({} reads, {} distinct responses, {} all-zero reads). Completeness and address advancement are unverified; contents were not displayed and this is not a restorable backup.",
        bytes.len(),
        reads.len(),
        unique_reads,
        all_zero_reads,
    );
    Ok(())
}

async fn selected_aula_macro_route(
    client: &openhid_ipc::AgentClient,
    args: &AulaArgs,
) -> Result<openhid_core::hid::DeviceRoute> {
    let snapshot = crate::agent::snapshot(client).await?;
    let candidates = snapshot
        .standalone
        .iter()
        .filter(|device| {
            openhid_device_registry::aula::matches_control(
                device.address.vendor_id,
                device.address.product_id,
                device.address.usage_page,
                device.address.usage_id,
            )
        })
        .collect::<Vec<_>>();
    let target = match args.identity.as_deref() {
        Some(identity) => candidates
            .iter()
            .copied()
            .find(|device| device.address.identity == identity)
            .with_context(|| {
                format!("no AULA F87 PRO receiver route matches identity {identity}")
            })?,
        None => match candidates.as_slice() {
            [target] => *target,
            [] => anyhow::bail!("the running Agent has no AULA F87 PRO receiver in its inventory"),
            _ => anyhow::bail!(
                "multiple AULA F87 PRO receivers are present; pass --identity from `openhid list`"
            ),
        },
    };
    anyhow::ensure!(
        target.online,
        "the AULA F87 PRO receiver is offline in the running Agent's latest inventory"
    );
    Ok(target.route())
}

async fn run_oem_matrix_probe(args: &AulaArgs) -> Result<()> {
    let matrix = openhid_hid::read_aula_matrix_oem_carrier(args.identity.as_deref()).await?;
    if let Some(path) = args.backup.as_deref() {
        write_backup(path, &matrix)?;
        println!(
            "OEM-carrier base matrix response backed up to {} ({} entries; the request changed no settings).",
            path.display(),
            matrix.entries().len()
        );
        return Ok(());
    }
    println!(
        "OEM-carrier base matrix query returned {} entries; no settings were changed.",
        matrix.entries().len()
    );
    for (index, entry) in matrix.entries().enumerate() {
        let label = AULA_F87_KEY_POSITIONS
            .iter()
            .find(|key| usize::from(key.matrix_slot()) == index)
            .map_or("unmapped", |key| key.label());
        println!(
            "{index:03} {label:<10} {:02x} {:02x} {:02x} {:02x}",
            entry[0], entry[1], entry[2], entry[3]
        );
    }
    Ok(())
}

async fn run_oem_keymap_probe(args: &AulaArgs) -> Result<()> {
    let keymap = openhid_hid::read_aula_keymap_oem_carrier(args.identity.as_deref()).await?;
    if let Some(path) = args.backup.as_deref() {
        write_backup(path, &AulaKeymapProfile::new(keymap.clone()))?;
        println!(
            "OEM-carrier base+Fn keymap backed up to {} ({} base entries, {} Fn entries; no settings were changed).",
            path.display(),
            keymap.base().len(),
            keymap.fn_layer().len()
        );
        return Ok(());
    }
    println!(
        "OEM-carrier keymap query returned {} base entries and {} Fn entries; no settings were changed.",
        keymap.base().len(),
        keymap.fn_layer().len()
    );
    for (layer, entries) in [("base", keymap.base()), ("fn", keymap.fn_layer())] {
        for (index, entry) in entries.iter().enumerate() {
            let label = AULA_F87_KEY_POSITIONS
                .iter()
                .find(|key| usize::from(key.matrix_slot()) == index)
                .map_or("unmapped", |key| key.label());
            let action = AulaKeymapAction::from_raw(*entry).map_or("", |action| action.label());
            println!(
                "{layer} {index:03} {label:<10} {:02x} {:02x} {:02x} {:02x} {action}",
                entry[0], entry[1], entry[2], entry[3]
            );
        }
    }
    Ok(())
}

async fn run_keymap(args: &AulaArgs) -> Result<()> {
    let keymap = openhid_hid::read_aula_keymap(args.identity.as_deref()).await?;
    if let Some(path) = args.backup.as_deref() {
        write_backup(path, &AulaKeymapProfile::new(keymap.clone()))?;
        println!(
            "Wireless base+Fn keymap backed up to {} ({} base entries, {} Fn entries).",
            path.display(),
            keymap.base().len(),
            keymap.fn_layer().len()
        );
        return Ok(());
    }
    println!(
        "Wireless keymap: {} base entries and {} Fn entries.",
        keymap.base().len(),
        keymap.fn_layer().len()
    );
    for (layer, entries) in [
        ("base", keymap.base().iter().collect::<Vec<_>>()),
        ("fn", keymap.fn_layer().iter().collect::<Vec<_>>()),
    ] {
        for (index, entry) in entries.iter().enumerate() {
            let label = AULA_F87_KEY_POSITIONS
                .iter()
                .find(|key| usize::from(key.matrix_slot()) == index)
                .map_or("unmapped", |key| key.label());
            let action = AulaKeymapAction::from_raw(**entry).map_or("", |action| action.label());
            println!(
                "{layer} {index:03} {label:<10} {:02x} {:02x} {:02x} {:02x} {action}",
                entry[0], entry[1], entry[2], entry[3]
            );
        }
    }
    Ok(())
}

async fn restore_keymap(path: &Path, identity: Option<&str>) -> Result<()> {
    let encoded = std::fs::read(path)
        .with_context(|| format!("read AULA keymap backup {}", path.display()))?;
    let backup = serde_json::from_slice::<AulaKeymapProfile>(&encoded)
        .context("invalid or incomplete F87 PRO keymap backup")?;
    let keymap = backup.keymap().clone();
    let actual = openhid_hid::update_aula_keymap(identity, keymap).await?;
    println!(
        "AULA base+Fn keymap written and verified ({} base entries, {} Fn entries).",
        actual.base().len(),
        actual.fn_layer().len()
    );
    Ok(())
}

fn plan_keymap_write(path: &Path) -> Result<()> {
    let encoded = std::fs::read(path)
        .with_context(|| format!("read AULA keymap profile {}", path.display()))?;
    let profile = serde_json::from_slice::<AulaKeymapProfile>(&encoded)
        .context("invalid or incomplete F87 PRO keymap profile")?;
    let plan = prepare_keymap_write_plan(profile.keymap())
        .context("keymap exceeds the F87 PRO OEM fragment-index range")?;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, &plan)?;
    output.write_all(b"\n")?;
    Ok(())
}

fn import_oem_profile(profile_path: &Path, export_path: &Path) -> Result<()> {
    let profile_bytes = std::fs::read(profile_path)
        .with_context(|| format!("read AULA OEM profile {}", profile_path.display()))?;
    let fn_layer = AulaOemFnLayer::from_profile_dct(&profile_bytes)
        .context("unsupported or invalid F87 PRO OEM profile.dct")?;
    let base_layer = AulaOemBaseLayer::from_profile_dct(&profile_bytes)
        .context("invalid F87 PRO OEM base layer")?;
    let keymap = AulaKeymap::new(base_layer.entries().to_vec(), fn_layer.entries().to_vec())
        .context("OEM profile did not produce a complete F87 PRO keymap")?;
    write_backup(
        export_path,
        &AulaKeymapProfile::with_oem_fn_data(
            keymap,
            Some(fn_layer.records().clone()),
            Some(fn_layer.actions().clone()),
        ),
    )?;

    println!(
        "Imported the OEM profile's 87 physical-key base and Fn assignments and opaque Fn fields to {}. No hardware was accessed; other OEM profile sections were not imported.",
        export_path.display()
    );
    Ok(())
}

async fn apply_settings(args: &AulaArgs, editing: bool) -> Result<()> {
    let lighting = if editing {
        let change = LightingChange::new(args.effect, args.brightness, args.speed)?;
        let color_mode = args
            .color
            .map(AulaColorModeChange::SingleColor)
            .or_else(|| (args.colorful > 0).then_some(AulaColorModeChange::Colorful));
        Some(match color_mode {
            Some(color_mode) => change.with_color_mode(color_mode)?,
            None => change,
        })
    } else {
        None
    };
    let sleep = args.sleep.and_then(SleepTimeout::from_seconds);
    let debounce = args.debounce.and_then(AulaDebounce::from_milliseconds);
    let settings =
        AulaChange::new_with_side_light(lighting, sleep, debounce, args.side_light_effect)?;
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
        println!("AULA settings edit verified; original full configuration restored and verified.");
    } else if args.color.is_some() || args.colorful > 0 {
        println!(
            "AULA lighting configuration readback verified. The color palette was acknowledged but has no verified readback."
        );
    } else if args.save {
        println!("AULA settings edit saved and verified by full configuration readback.");
    } else {
        println!("AULA settings edit staged and verified (no SAVE command).");
    }
    Ok(())
}

async fn apply_rgb_profile(path: &Path, identity: Option<&str>) -> Result<()> {
    let encoded =
        std::fs::read(path).with_context(|| format!("read AULA RGB profile {}", path.display()))?;
    let profile = serde_json::from_slice::<AulaPerKeyRgbProfile>(&encoded)
        .context("invalid or unsupported AULA per-key RGB profile")?;
    let state = openhid_hid::start_aula_per_key_rgb(identity, profile.colors().clone())
        .await?
        .finish()
        .await?;
    println!(
        "AULA per-key RGB table sent and saved; configuration readback reports effect {}. The color table itself has no verified readback.",
        state.lighting.effect
    );
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

fn write_raw_dump(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "create raw macro-prefix dump {} (existing files are preserved)",
                path.display()
            )
        })?;
    file.write_all(bytes)?;
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
