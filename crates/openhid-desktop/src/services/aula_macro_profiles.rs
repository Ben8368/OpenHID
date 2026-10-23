//! Local named profiles for offline AULA OEM macro files.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use openhid_core::hid::aula::AulaOemMacroProfile;

use super::aula_profiles;

/// One validated macro profile stored in OpenHID's local data directory.
#[derive(Debug, Clone)]
pub(crate) struct SavedAulaMacroProfile {
    /// User-facing profile name.
    pub name: String,
    /// Preserved OEM header and opaque event records.
    pub profile: AulaOemMacroProfile,
}

/// Load valid macro profiles, skipping damaged local files.
pub(crate) fn load_all() -> Result<Vec<SavedAulaMacroProfile>> {
    let directory = profile_directory()?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("read {}", directory.display())),
    };
    let mut profiles = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("read entry in {}", directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(aula_profiles::decode_name)
        else {
            tracing::warn!(path = %path.display(), "skipping an invalid AULA macro profile filename");
            continue;
        };
        match read_profile(&path) {
            Ok(profile) => profiles.push(SavedAulaMacroProfile { name, profile }),
            Err(error) => tracing::warn!(%error, "skipping an invalid local AULA macro profile"),
        }
    }
    profiles.sort_by_key(|profile| profile.name.to_lowercase());
    Ok(profiles)
}

/// Import an OEM `.mcr` file or portable OpenHID JSON profile under a unique name.
pub(crate) fn import(path: &Path) -> Result<SavedAulaMacroProfile> {
    let bytes = fs::read(path).with_context(|| format!("read AULA macro {}", path.display()))?;
    let profile = match path.extension().and_then(|ext| ext.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("mcr") => {
            AulaOemMacroProfile::from_oem_bytes(&bytes).context("parse AULA OEM macro")?
        }
        _ => serde_json::from_slice::<AulaOemMacroProfile>(&bytes)
            .with_context(|| format!("parse AULA macro profile {}", path.display()))?,
    };
    let base_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|name| aula_profiles::validate_name(name).is_ok())
        .unwrap_or("Imported AULA macro");
    let existing = load_all()?
        .into_iter()
        .map(|saved| saved.name)
        .collect::<Vec<_>>();
    let name = aula_profiles::unique_name(base_name, &existing);
    save(&name, &profile)?;
    Ok(SavedAulaMacroProfile { name, profile })
}

/// Save or replace a named validated macro profile.
pub(crate) fn save(name: &str, profile: &AulaOemMacroProfile) -> Result<()> {
    aula_profiles::validate_name(name)?;
    let directory = profile_directory()?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join(format!("{}.json", aula_profiles::encode_name(name)));
    let encoded = serde_json::to_vec_pretty(profile).context("encode AULA macro profile")?;
    fs::write(&path, encoded)
        .with_context(|| format!("write AULA macro profile {}", path.display()))
}

/// Export a profile as OEM `.mcr` bytes or portable JSON, without overwriting.
pub(crate) fn export(profile: &AulaOemMacroProfile, path: &Path) -> Result<()> {
    let bytes = if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mcr"))
    {
        profile.to_oem_bytes()
    } else {
        serde_json::to_vec_pretty(profile).context("encode AULA macro export")?
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create AULA macro export {}", path.display()))?;
    if let Err(error) = file.write_all(&bytes) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error).with_context(|| format!("write AULA macro export {}", path.display()));
    }
    Ok(())
}

/// Create and return the local macro profile directory for native file pickers.
pub(crate) fn ensure_directory() -> Result<PathBuf> {
    let directory = profile_directory()?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    Ok(directory)
}

/// Remove one named local profile.
pub(crate) fn delete(name: &str) -> Result<()> {
    aula_profiles::validate_name(name)?;
    let path = profile_directory()?.join(format!("{}.json", aula_profiles::encode_name(name)));
    fs::remove_file(&path).with_context(|| format!("delete AULA macro profile {}", path.display()))
}

fn profile_directory() -> Result<PathBuf> {
    aula_profiles::profile_directory_for("aula-macro-profiles")
}

fn read_profile(path: &Path) -> Result<AulaOemMacroProfile> {
    let bytes =
        fs::read(path).with_context(|| format!("read AULA macro profile {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parse AULA macro profile {}", path.display()))
}
