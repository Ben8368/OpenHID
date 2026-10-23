//! Local named F87 wireless keymap profiles.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::Path,
};

use anyhow::{Context as _, Result};
use openhid_core::hid::aula::{
    AulaKeymap, AulaKeymapProfile, AulaOemBaseLayer, AulaOemFnActions, AulaOemFnLayer,
    AulaOemFnRecords,
};

use super::aula_profiles;

/// A validated named base+Fn profile stored by OpenHID on this computer.
#[derive(Debug, Clone)]
pub(crate) struct SavedAulaKeymapProfile {
    /// User-facing profile name.
    pub name: String,
    /// Exact validated raw keymap table.
    pub keymap: AulaKeymap,
    /// Opaque per-key fields retained from an imported OEM profile.
    pub oem_fn_records: Option<AulaOemFnRecords>,
    /// Original OEM Fn actions retained to disambiguate opaque action encodings.
    pub oem_fn_actions: Option<AulaOemFnActions>,
}

/// Load every valid local keymap profile, skipping damaged files.
pub(crate) fn load_all() -> Result<Vec<SavedAulaKeymapProfile>> {
    let directory = aula_profiles::profile_directory_for("aula-keymap-profiles")?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("read {}", directory.display())),
    };
    let mut profiles = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("read profile entry in {}", directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(aula_profiles::decode_name)
        else {
            tracing::warn!(path = %path.display(), "skipping an invalid AULA keymap profile filename");
            continue;
        };
        match read_profile(&path) {
            Ok(profile) => profiles.push(SavedAulaKeymapProfile {
                name,
                keymap: profile.keymap().clone(),
                oem_fn_records: profile.oem_fn_records().cloned(),
                oem_fn_actions: profile.oem_fn_actions().cloned(),
            }),
            Err(error) => tracing::warn!(%error, "skipping an invalid local AULA keymap profile"),
        }
    }
    profiles.sort_by_key(|profile| profile.name.to_lowercase());
    Ok(profiles)
}

/// Save or replace one named profile without touching the keyboard.
pub(crate) fn save(
    name: &str,
    keymap: &AulaKeymap,
    oem_fn_records: Option<&AulaOemFnRecords>,
    oem_fn_actions: Option<&AulaOemFnActions>,
) -> Result<()> {
    aula_profiles::validate_name(name)?;
    let directory = profile_directory()?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join(format!("{}.json", aula_profiles::encode_name(name)));
    let profile = AulaKeymapProfile::with_oem_fn_data(
        keymap.clone(),
        oem_fn_records.cloned(),
        oem_fn_actions.cloned(),
    );
    let encoded = serde_json::to_vec_pretty(&profile).context("encode AULA keymap profile")?;
    fs::write(&path, encoded)
        .with_context(|| format!("write AULA keymap profile {}", path.display()))
}

/// Import a validated keymap and select a collision-free local name.
pub(crate) fn import(path: &Path) -> Result<SavedAulaKeymapProfile> {
    let profile = read_profile(path)?;
    let keymap = profile.keymap().clone();
    let oem_fn_records = profile.oem_fn_records().cloned();
    let oem_fn_actions = profile.oem_fn_actions().cloned();
    let base_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|name| aula_profiles::validate_name(name).is_ok())
        .unwrap_or("Imported AULA keymap");
    let existing = load_all()?
        .into_iter()
        .map(|profile| profile.name)
        .collect::<Vec<_>>();
    let name = aula_profiles::unique_name(base_name, &existing);
    save(
        &name,
        &keymap,
        oem_fn_records.as_ref(),
        oem_fn_actions.as_ref(),
    )?;
    Ok(SavedAulaKeymapProfile {
        name,
        keymap,
        oem_fn_records,
        oem_fn_actions,
    })
}

/// Import both keymap layers from the OEM's local `profile.dct` file.
pub(crate) fn import_oem_profile(path: &Path) -> Result<SavedAulaKeymapProfile> {
    let bytes =
        fs::read(path).with_context(|| format!("read AULA OEM profile {}", path.display()))?;
    let fn_layer = AulaOemFnLayer::from_profile_dct(&bytes)
        .with_context(|| format!("parse AULA OEM profile {}", path.display()))?;
    let base_layer = AulaOemBaseLayer::from_profile_dct(&bytes)
        .with_context(|| format!("parse AULA OEM base layer {}", path.display()))?;
    let keymap = AulaKeymap::new(base_layer.entries().to_vec(), fn_layer.entries().to_vec())
        .context("OEM profile did not produce a complete F87 PRO keymap")?;
    let oem_fn_records = fn_layer.records().clone();
    let oem_fn_actions = fn_layer.actions().clone();
    let base_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|name| aula_profiles::validate_name(name).is_ok())
        .unwrap_or("Imported AULA OEM profile");
    let existing = load_all()?
        .into_iter()
        .map(|profile| profile.name)
        .collect::<Vec<_>>();
    let name = aula_profiles::unique_name(base_name, &existing);
    save(&name, &keymap, Some(&oem_fn_records), Some(&oem_fn_actions))?;
    Ok(SavedAulaKeymapProfile {
        name,
        keymap,
        oem_fn_records: Some(oem_fn_records),
        oem_fn_actions: Some(oem_fn_actions),
    })
}

/// Export a selected keymap as validated JSON without overwriting another file.
pub(crate) fn export(
    keymap: &AulaKeymap,
    oem_fn_records: Option<&AulaOemFnRecords>,
    oem_fn_actions: Option<&AulaOemFnActions>,
    path: &Path,
) -> Result<()> {
    let profile = AulaKeymapProfile::with_oem_fn_data(
        keymap.clone(),
        oem_fn_records.cloned(),
        oem_fn_actions.cloned(),
    );
    let encoded = serde_json::to_vec_pretty(&profile).context("encode AULA keymap export")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create AULA keymap export {}", path.display()))?;
    if let Err(error) = file.write_all(&encoded) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error).with_context(|| format!("write AULA keymap export {}", path.display()));
    }
    Ok(())
}

/// Export a keymap's Fn layer into a validated OEM DCT template.
pub(crate) fn export_oem_profile(
    keymap: &AulaKeymap,
    oem_fn_records: Option<&AulaOemFnRecords>,
    oem_fn_actions: Option<&AulaOemFnActions>,
    template_path: &Path,
    output_path: &Path,
) -> Result<()> {
    let template = fs::read(template_path)
        .with_context(|| format!("read AULA OEM profile template {}", template_path.display()))?;
    let bytes = openhid_core::hid::aula::write_keymap_to_profile_dct_with_fn_actions(
        &template,
        keymap,
        oem_fn_records,
        oem_fn_actions,
    )
    .with_context(|| format!("update keymap mappings in {}", template_path.display()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
        .with_context(|| {
            format!(
                "create OEM profile export {} (existing files are preserved)",
                output_path.display()
            )
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(output_path);
        return Err(error).with_context(|| format!("write OEM profile {}", output_path.display()));
    }
    Ok(())
}

/// Create and return the local keymap profile directory.
pub(crate) fn ensure_directory() -> Result<std::path::PathBuf> {
    let directory = profile_directory()?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    Ok(directory)
}

/// Remove a named local profile.
pub(crate) fn delete(name: &str) -> Result<()> {
    aula_profiles::validate_name(name)?;
    let path = profile_directory()?.join(format!("{}.json", aula_profiles::encode_name(name)));
    fs::remove_file(&path).with_context(|| format!("delete AULA keymap profile {}", path.display()))
}

fn profile_directory() -> Result<std::path::PathBuf> {
    aula_profiles::profile_directory_for("aula-keymap-profiles")
}

fn read_profile(path: &Path) -> Result<AulaKeymapProfile> {
    let bytes =
        fs::read(path).with_context(|| format!("read AULA keymap profile {}", path.display()))?;
    let profile = serde_json::from_slice::<AulaKeymapProfile>(&bytes)
        .with_context(|| format!("parse AULA keymap profile {}", path.display()))?;
    Ok(profile)
}
