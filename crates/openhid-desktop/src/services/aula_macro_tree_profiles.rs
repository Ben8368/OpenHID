//! Local named profiles for offline AULA OEM macro trees.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use openhid_core::hid::aula::{
    AULA_OEM_MACRO_TREE_PROFILE_FORMAT, AulaOemMacroTree, AulaOemMacroTreeProfile,
};

use super::aula_profiles;

/// One validated macro tree stored in OpenHID's local data directory.
#[derive(Debug, Clone)]
pub(crate) struct SavedAulaMacroTreeProfile {
    /// User-facing profile name.
    pub name: String,
    /// Preserved OEM node headers and macro event records.
    pub profile: AulaOemMacroTreeProfile,
}

/// Load valid local macro trees, skipping damaged profile files.
pub(crate) fn load_all() -> Result<Vec<SavedAulaMacroTreeProfile>> {
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
            tracing::warn!(path = %path.display(), "skipping an invalid AULA macro-tree profile filename");
            continue;
        };
        match read_profile(&path) {
            Ok(profile) => profiles.push(SavedAulaMacroTreeProfile { name, profile }),
            Err(error) => {
                tracing::warn!(%error, "skipping an invalid local AULA macro-tree profile");
            }
        }
    }
    profiles.sort_by_key(|profile| profile.name.to_lowercase());
    Ok(profiles)
}

/// Import a local `.dct` tree or portable JSON profile under a unique name.
pub(crate) fn import(path: &Path) -> Result<SavedAulaMacroTreeProfile> {
    let bytes =
        fs::read(path).with_context(|| format!("read AULA macro tree {}", path.display()))?;
    let profile = match path.extension().and_then(|ext| ext.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("dct") => {
            AulaOemMacroTreeProfile::from_dct_bytes(&bytes).context("parse AULA macro tree")?
        }
        _ => serde_json::from_slice::<AulaOemMacroTreeProfile>(&bytes)
            .with_context(|| format!("parse AULA macro-tree profile {}", path.display()))?,
    };
    let base_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|name| aula_profiles::validate_name(name).is_ok())
        .unwrap_or("Imported AULA macro tree");
    let existing = load_all()?
        .into_iter()
        .map(|saved| saved.name)
        .collect::<Vec<_>>();
    let name = aula_profiles::unique_name(base_name, &existing);
    save(&name, &profile)?;
    Ok(SavedAulaMacroTreeProfile { name, profile })
}

/// Create an empty local macro tree under a new user-provided name.
pub(crate) fn create_empty(name: &str) -> Result<SavedAulaMacroTreeProfile> {
    aula_profiles::validate_name(name)?;
    let existing_names = load_all()?
        .into_iter()
        .map(|saved| saved.name)
        .collect::<Vec<_>>();
    let name = aula_profiles::unique_name(name, &existing_names);
    let tree = AulaOemMacroTree::new(Vec::new()).context("create empty AULA macro tree")?;
    let profile = AulaOemMacroTreeProfile::new(tree);
    save(&name, &profile)?;
    Ok(SavedAulaMacroTreeProfile { name, profile })
}

/// Identify local tree files and portable tree profiles in the shared JSON picker.
pub(crate) fn is_tree_input(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dct"))
    {
        return true;
    }
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        return false;
    }
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|profile| {
            profile
                .get("format")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .is_some_and(|format| format == AULA_OEM_MACRO_TREE_PROFILE_FORMAT)
}

/// Save or replace one named local macro-tree profile.
pub(crate) fn save(name: &str, profile: &AulaOemMacroTreeProfile) -> Result<()> {
    aula_profiles::validate_name(name)?;
    let directory = profile_directory()?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join(format!("{}.json", aula_profiles::encode_name(name)));
    let encoded = serde_json::to_vec_pretty(profile).context("encode AULA macro-tree profile")?;
    fs::write(&path, encoded)
        .with_context(|| format!("write AULA macro-tree profile {}", path.display()))
}

/// Export a profile as local `.dct` bytes or portable JSON, without overwriting.
pub(crate) fn export(profile: &AulaOemMacroTreeProfile, path: &Path) -> Result<()> {
    let bytes = if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dct"))
    {
        profile.to_dct_bytes()
    } else {
        serde_json::to_vec_pretty(profile).context("encode AULA macro-tree export")?
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create AULA macro-tree export {}", path.display()))?;
    if let Err(error) = file.write_all(&bytes) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error)
            .with_context(|| format!("write AULA macro-tree export {}", path.display()));
    }
    Ok(())
}

/// Create the local macro-tree directory for native file pickers.
pub(crate) fn ensure_directory() -> Result<PathBuf> {
    let directory = profile_directory()?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    Ok(directory)
}

/// Remove one named local macro-tree profile.
pub(crate) fn delete(name: &str) -> Result<()> {
    aula_profiles::validate_name(name)?;
    let path = profile_directory()?.join(format!("{}.json", aula_profiles::encode_name(name)));
    fs::remove_file(&path)
        .with_context(|| format!("delete AULA macro-tree profile {}", path.display()))
}

fn profile_directory() -> Result<PathBuf> {
    aula_profiles::profile_directory_for("aula-macro-tree-profiles")
}

fn read_profile(path: &Path) -> Result<AulaOemMacroTreeProfile> {
    let bytes = fs::read(path)
        .with_context(|| format!("read AULA macro-tree profile {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parse AULA macro-tree profile {}", path.display()))
}
