//! AULA keymap profile editing and wired Feature Report upload.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use clap::Subcommand;
use openhid_core::hid::aula::{
    AULA_F87_KEY_POSITIONS, AULA_WIRED_KEYMAP_SLOT_COUNT, AulaBaseMatrix, AulaKeymap,
    AulaKeymapProfile, AulaOemFnLayer, AulaWiredKeymapBlob, AulaWiredKeymapLayer,
    aula_f87_oem_default_fn_actions, aula_f87_oem_default_fn_layer, aula_keymap_slot_is_assignable,
    aula_wired_keymap_action_output, aula_wired_keymap_action_outputs,
    aula_wired_keymap_base_from_oem_profile, aula_wired_keymap_factory_template,
    aula_wired_keymap_slot_from_matrix_action, write_keymap_to_profile_dct_with_fn_actions,
};
use openhid_fixture::format_hex;

/// Offline AULA keymap profile editing and an explicit wired Feature Report upload.
#[derive(Debug, Subcommand)]
pub enum AulaKeymapCmd {
    /// Export the packaged F87 wired factory keymap layer as an editable 520-byte report.
    FactoryTemplate {
        /// Layer to seed: `base` contains factory assignments; `fn` is unbound.
        #[arg(long, value_parser = parse_layer)]
        layer: AulaWiredKeymapLayer,
        /// New report path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Convert supported F87 OEM base assignments into a wired keymap report.
    ImportOemBase {
        /// Installed F87 PRO `profile.dct` file.
        #[arg(long)]
        input: PathBuf,
        /// New 520-byte base-layer report; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Report which installed OEM Fn assignments fit the wired single-usage format.
    InspectOemFn {
        /// Installed F87 PRO `profile.dct` file.
        #[arg(long)]
        input: PathBuf,
    },
    /// Persist one validated wired keymap layer to an explicitly selected F87 Pro collection.
    WriteWired {
        /// Validated 520-byte wired Feature Report for one layer.
        #[arg(long)]
        input: PathBuf,
        /// Exact identity printed by `openhid diag aula --wired-model-probe`.
        #[arg(long)]
        identity: String,
        /// Confirm the persistent keymap upload; the device does not provide reliable readback.
        #[arg(long)]
        apply: bool,
    },
    /// Replace the Fn layer in a local wireless profile with F87 OEM defaults.
    RestoreFnDefaults {
        /// Wireless keymap profile or base-matrix JSON backup; the base layer is retained.
        #[arg(long)]
        input: PathBuf,
        /// New output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Export a wireless keymap profile into a new OEM `profile.dct` file.
    ExportOemProfile {
        /// Validated OpenHID wireless keymap profile JSON.
        #[arg(long)]
        input: PathBuf,
        /// Existing OEM `profile.dct` used as the byte-preserving template.
        #[arg(long)]
        template: PathBuf,
        /// New OEM profile output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Validate a raw report and print each complete slot without device access.
    Inspect {
        /// Captured 520-byte report blob, including report ID byte.
        #[arg(long)]
        input: PathBuf,
    },
    /// Replace one four-byte slot while preserving every other input byte.
    ReplaceSlot {
        /// Captured or previously edited 520-byte report blob.
        #[arg(long)]
        input: PathBuf,
        /// New output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based matrix/LED slot index.
        #[arg(long)]
        index: usize,
        /// Exactly eight hexadecimal digits for the slot's four raw bytes.
        #[arg(long)]
        bytes: String,
    },
    /// Replace a named physical key slot while preserving every other input byte.
    ReplaceKey {
        /// Captured or previously edited 520-byte report blob.
        #[arg(long)]
        input: PathBuf,
        /// New output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// OEM key legend from the F87 PRO layout, such as `A`, `Space`, or `Right`.
        #[arg(long)]
        key: String,
        /// Exactly eight hexadecimal digits for the slot's four raw bytes.
        #[arg(long)]
        bytes: String,
    },
    /// Replace a named physical key slot with a shared named action.
    SetAction {
        /// Captured or previously edited 520-byte report blob.
        #[arg(long)]
        input: PathBuf,
        /// New output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// OEM key legend from the F87 PRO layout, such as `A`, `Space`, or `Right`.
        #[arg(long)]
        key: String,
        /// Wired single-key action, such as `f24`, `media_play`, or `unbound`.
        #[arg(long)]
        action: String,
    },
    /// List action identifiers and raw bytes available to the editors.
    ListActions,
    /// Select the base or Fn layer while preserving every other input byte.
    SetLayer {
        /// Captured or previously edited 520-byte report blob.
        #[arg(long)]
        input: PathBuf,
        /// New output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Layer to encode in header byte 2.
        #[arg(long, value_parser = parse_layer)]
        layer: AulaWiredKeymapLayer,
    },
}

impl AulaKeymapCmd {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::FactoryTemplate { layer, output } => export_factory_template(layer, &output)?,
            Self::ImportOemBase { input, output } => import_oem_base(&input, &output)?,
            Self::InspectOemFn { input } => inspect_oem_fn(&input)?,
            Self::WriteWired {
                input,
                identity,
                apply,
            } => {
                anyhow::ensure!(
                    apply,
                    "wired keymap upload writes persistently; review the report and pass --apply to continue"
                );
                let blob = read_blob(&input)?;
                openhid_hid::write_aula_wired_keymap(Some(&identity), &blob).await?;
                println!(
                    "AULA wired {:?} keymap report uploaded. The keyboard does not provide reliable keymap readback; verify key behavior before relying on this mapping.",
                    blob.layer()
                );
            }
            Self::RestoreFnDefaults { input, output } => restore_fn_defaults(&input, &output)?,
            Self::ExportOemProfile {
                input,
                template,
                output,
            } => export_oem_profile(&input, &template, &output)?,
            Self::Inspect { input } => {
                let blob = read_blob(&input)?;
                println!("layer: {:?}", blob.layer());
                println!("complete slots: {AULA_WIRED_KEYMAP_SLOT_COUNT}");
                for index in 0..AULA_WIRED_KEYMAP_SLOT_COUNT {
                    let bytes = blob
                        .slot(index)
                        .with_context(|| format!("missing complete slot {index}"))?;
                    let label = AULA_F87_KEY_POSITIONS
                        .iter()
                        .find(|key| usize::from(key.matrix_slot()) == index)
                        .map_or("unmapped", |key| key.label());
                    println!("{index:03} {label:<10} {}", format_hex(&bytes));
                }
            }
            Self::ReplaceSlot {
                input,
                output,
                index,
                bytes,
            } => {
                let mut blob = read_blob(&input)?;
                let slot = parse_slot(&bytes)?;
                blob.replace_slot(index, slot)?;
                write_new_file(&output, blob.as_bytes())?;
            }
            Self::ReplaceKey {
                input,
                output,
                key,
                bytes,
            } => {
                let mut blob = read_blob(&input)?;
                replace_named_key(&mut blob, &key, parse_slot(&bytes)?)?;
                write_new_file(&output, blob.as_bytes())?;
            }
            Self::SetAction {
                input,
                output,
                key,
                action,
            } => {
                let raw = aula_wired_keymap_action_output(&action).with_context(|| {
                    format!(
                        "unknown or unsupported wired action `{action}`; use `openhid aula-keymap list-actions`"
                    )
                })?;
                let mut blob = read_blob(&input)?;
                replace_named_key(&mut blob, &key, raw)?;
                write_new_file(&output, blob.as_bytes())?;
            }
            Self::ListActions => {
                for (name, label, raw) in aula_wired_keymap_action_outputs() {
                    println!("{:<24} {:<24} {}", name, label, format_hex(&raw));
                }
            }
            Self::SetLayer {
                input,
                output,
                layer,
            } => {
                let mut blob = read_blob(&input)?;
                blob.set_layer(layer);
                write_new_file(&output, blob.as_bytes())?;
            }
        }
        Ok(())
    }
}

fn export_factory_template(layer: AulaWiredKeymapLayer, output: &Path) -> Result<()> {
    let blob = aula_wired_keymap_factory_template(layer)?;
    write_new_file(output, blob.as_bytes())?;
    println!(
        "F87 wired {layer:?} factory template exported to {} (no device I/O).",
        output.display()
    );
    if layer == AulaWiredKeymapLayer::Fn {
        println!("The template is unbound; uploading it clears Fn-layer assignments.");
    }
    Ok(())
}

fn import_oem_base(input: &Path, output: &Path) -> Result<()> {
    let profile = std::fs::read(input)
        .with_context(|| format!("read F87 OEM profile {}", input.display()))?;
    let blob = aula_wired_keymap_base_from_oem_profile(&profile)
        .context("the OEM profile contains a base action the wired report cannot encode")?;
    write_new_file(output, blob.as_bytes())?;
    println!(
        "F87 OEM base assignments converted to a wired report at {} (no device I/O; reserved F1–F12 slots and Fn layer were not changed).",
        output.display()
    );
    Ok(())
}

fn inspect_oem_fn(input: &Path) -> Result<()> {
    let profile = std::fs::read(input)
        .with_context(|| format!("read F87 OEM profile {}", input.display()))?;
    let fn_layer =
        AulaOemFnLayer::from_profile_dct(&profile).context("decode F87 OEM Fn assignments")?;
    let mut representable = 0;
    let mut unsupported = 0;
    for (record_index, position) in AULA_F87_KEY_POSITIONS.iter().enumerate() {
        let source = fn_layer
            .actions()
            .action(record_index)
            .context("missing OEM Fn action record")?;
        let matrix = fn_layer.entries()[usize::from(position.matrix_slot())];
        if aula_wired_keymap_slot_from_matrix_action(matrix).is_some() {
            representable += 1;
        } else {
            unsupported += 1;
            println!(
                "unsupported {:<10} raw={} matrix={}",
                position.label(),
                format_hex(&source),
                format_hex(&matrix)
            );
        }
    }
    println!(
        "F87 OEM Fn audit: {representable} of {} physical assignments fit the wired single-usage format; {unsupported} require protocol mapping. No device I/O was performed.",
        AULA_F87_KEY_POSITIONS.len()
    );
    Ok(())
}

fn restore_fn_defaults(input: &Path, output: &Path) -> Result<()> {
    let encoded = std::fs::read(input)
        .with_context(|| format!("read AULA keymap profile {}", input.display()))?;
    let value = serde_json::from_slice::<serde_json::Value>(&encoded)
        .context("invalid F87 PRO keymap or base-matrix JSON")?;
    let (base, oem_fn_records) = if value.get("fn_layer").is_some() {
        let profile = serde_json::from_value::<AulaKeymapProfile>(value)
            .context("invalid or incomplete F87 PRO wireless keymap profile")?;
        (
            profile.keymap().base().to_vec(),
            profile.oem_fn_records().cloned(),
        )
    } else {
        let matrix = serde_json::from_value::<AulaBaseMatrix>(value)
            .context("input is neither a wireless keymap profile nor base-matrix backup")?;
        (matrix.entries().to_vec(), None)
    };
    let keymap = AulaKeymap::new(base, aula_f87_oem_default_fn_layer())
        .context("OEM Fn defaults did not produce a complete F87 PRO keymap")?;
    let restored = AulaKeymapProfile::with_oem_fn_data(
        keymap,
        oem_fn_records,
        Some(aula_f87_oem_default_fn_actions()),
    );
    let encoded =
        serde_json::to_vec_pretty(&restored).context("encode F87 PRO OEM Fn-default profile")?;
    write_new_file(output, &encoded)?;
    println!(
        "F87 PRO OEM Fn defaults exported to {} (base layer retained; no device I/O).",
        output.display()
    );
    Ok(())
}

fn export_oem_profile(input: &Path, template: &Path, output: &Path) -> Result<()> {
    let encoded = std::fs::read(input)
        .with_context(|| format!("read AULA keymap profile {}", input.display()))?;
    let profile = serde_json::from_slice::<AulaKeymapProfile>(&encoded)
        .context("invalid or incomplete F87 PRO wireless keymap profile")?;
    let template_bytes = std::fs::read(template)
        .with_context(|| format!("read AULA OEM profile template {}", template.display()))?;
    let exported = write_keymap_to_profile_dct_with_fn_actions(
        &template_bytes,
        profile.keymap(),
        profile.oem_fn_records(),
        profile.oem_fn_actions(),
    )
    .with_context(|| {
        format!(
            "update keymap mappings in OEM profile {}",
            template.display()
        )
    })?;
    write_new_file(output, &exported)?;
    println!(
        "F87 PRO keymap exported to {} (OEM profile fields preserved; no device I/O).",
        output.display()
    );
    Ok(())
}

fn replace_named_key(blob: &mut AulaWiredKeymapBlob, key: &str, raw: [u8; 4]) -> Result<()> {
    let position = AULA_F87_KEY_POSITIONS
        .iter()
        .find(|position| position.label().eq_ignore_ascii_case(key.trim()))
        .with_context(|| format!("unknown F87 PRO key legend `{key}`"))?;
    let index = usize::from(position.matrix_slot());
    let is_fn_layer = blob.layer() == AulaWiredKeymapLayer::Fn;
    if !aula_keymap_slot_is_assignable(index, is_fn_layer) {
        bail!(
            "F87 PRO base-layer source `{}` is reserved",
            position.label()
        );
    }
    blob.replace_slot(index, raw)?;
    Ok(())
}

fn read_blob(path: &PathBuf) -> Result<AulaWiredKeymapBlob> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("read AULA wired keymap report {}", path.display()))?;
    AulaWiredKeymapBlob::from_bytes(bytes)
        .with_context(|| format!("validate AULA wired keymap report {}", path.display()))
}

fn parse_layer(value: &str) -> Result<AulaWiredKeymapLayer, String> {
    match value.to_ascii_lowercase().as_str() {
        "base" => Ok(AulaWiredKeymapLayer::Base),
        "fn" => Ok(AulaWiredKeymapLayer::Fn),
        _ => Err("layer must be `base` or `fn`".to_owned()),
    }
}

fn parse_slot(value: &str) -> Result<[u8; 4]> {
    let hex = value.strip_prefix("0x").unwrap_or(value);
    let digits = hex.as_bytes();
    if digits.len() != 8 {
        bail!("slot bytes must contain exactly eight hexadecimal digits");
    }
    let mut bytes = [0; 4];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        let high = hex_nibble(digits[start])
            .with_context(|| format!("invalid hexadecimal digit at offset {start}"))?;
        let low = hex_nibble(digits[start + 1])
            .with_context(|| format!("invalid hexadecimal digit at offset {}", start + 1))?;
        *byte = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_nibble(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create new file {}", path.display()))?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = std::fs::remove_file(path);
        return Err(error).with_context(|| format!("write {}", path.display()));
    }
    Ok(())
}
