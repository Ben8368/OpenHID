//! Local named F87 per-key RGB profiles.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use openhid_core::hid::aula::AulaPerKeyRgbProfile;

/// One validated, named profile from OpenHID's local data directory.
#[derive(Debug, Clone)]
pub(crate) struct SavedAulaProfile {
    /// User-facing name; the filename encodes its UTF-8 bytes safely.
    pub name: String,
    /// Complete 126-entry table.
    pub profile: AulaPerKeyRgbProfile,
}

/// Read all valid profiles. A damaged file is skipped without hiding others.
pub(crate) fn load_all() -> Result<Vec<SavedAulaProfile>> {
    let directory = profile_directory_for("aula-rgb-profiles")?;
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
            .and_then(decode_name)
        else {
            tracing::warn!(path = %path.display(), "skipping an invalid AULA profile filename");
            continue;
        };
        match fs::read(&path)
            .with_context(|| format!("read AULA profile {}", path.display()))
            .and_then(|bytes| {
                serde_json::from_slice::<AulaPerKeyRgbProfile>(&bytes)
                    .with_context(|| format!("parse AULA profile {}", path.display()))
            }) {
            Ok(profile) => profiles.push(SavedAulaProfile { name, profile }),
            Err(error) => tracing::warn!(%error, "skipping an invalid local AULA profile"),
        }
    }
    profiles.sort_by_key(|profile| profile.name.to_lowercase());
    Ok(profiles)
}

/// Save or replace one named profile as the same portable JSON accepted by CLI import.
pub(crate) fn save(name: &str, profile: &AulaPerKeyRgbProfile) -> Result<()> {
    validate_name(name)?;
    let directory = profile_directory_for("aula-rgb-profiles")?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join(format!("{}.json", encode_name(name)));
    let encoded = serde_json::to_vec_pretty(profile).context("encode AULA RGB profile")?;
    fs::write(&path, encoded).with_context(|| format!("write AULA profile {}", path.display()))
}

/// Import a versioned JSON profile, naming it from the selected file and
/// avoiding replacement when a local profile already has that name.
pub(crate) fn import(path: &Path) -> Result<SavedAulaProfile> {
    let bytes = fs::read(path).with_context(|| format!("read AULA profile {}", path.display()))?;
    let profile = serde_json::from_slice::<AulaPerKeyRgbProfile>(&bytes)
        .with_context(|| format!("parse AULA profile {}", path.display()))?;
    let base_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|name| validate_name(name).is_ok())
        .unwrap_or("Imported AULA profile");
    let existing = load_all()?
        .into_iter()
        .map(|saved| saved.name)
        .collect::<Vec<_>>();
    let name = unique_name(base_name, &existing);
    save(&name, &profile)?;
    Ok(SavedAulaProfile { name, profile })
}

/// Export a selected profile to a user-chosen JSON path.
pub(crate) fn export(profile: &AulaPerKeyRgbProfile, path: &Path) -> Result<()> {
    let encoded = serde_json::to_vec_pretty(profile).context("encode AULA RGB profile")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("create AULA profile export {}", path.display()))?;
    if let Err(error) = file.write_all(&encoded) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error).with_context(|| format!("write AULA profile export {}", path.display()));
    }
    Ok(())
}

/// Ensure the local profile directory exists before opening a native picker.
pub(crate) fn ensure_directory() -> Result<PathBuf> {
    let directory = profile_directory_for("aula-rgb-profiles")?;
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    Ok(directory)
}

/// Delete a named profile from OpenHID's local data directory.
pub(crate) fn delete(name: &str) -> Result<()> {
    validate_name(name)?;
    let path =
        profile_directory_for("aula-rgb-profiles")?.join(format!("{}.json", encode_name(name)));
    fs::remove_file(&path).with_context(|| format!("delete AULA profile {}", path.display()))
}

pub(super) fn profile_directory_for(folder: &str) -> Result<PathBuf> {
    Ok(openhid_core::paths::data_dir()?.join(folder))
}

pub(super) fn validate_name(name: &str) -> Result<()> {
    if name.trim() != name
        || name.is_empty()
        || name.chars().count() > 64
        || name.chars().any(char::is_control)
    {
        bail!(
            "profile names must be 1–64 characters with no leading/trailing whitespace or control characters"
        );
    }
    Ok(())
}

pub(super) fn encode_name(name: &str) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(name.len() * 2);
    for byte in name.bytes() {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

pub(super) fn decode_name(encoded: &str) -> Option<String> {
    if encoded.is_empty() || !encoded.len().is_multiple_of(2) {
        return None;
    }
    let (pairs, remainder) = encoded.as_bytes().as_chunks::<2>();
    if !remainder.is_empty() {
        return None;
    }
    let bytes = pairs
        .iter()
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            u8::try_from((high << 4) | low).ok()
        })
        .collect::<Option<Vec<_>>>()?;
    let name = String::from_utf8(bytes).ok()?;
    validate_name(&name).ok()?;
    Some(name)
}

pub(super) fn unique_name(base: &str, existing: &[String]) -> String {
    let collides = |candidate: &str| {
        existing
            .iter()
            .any(|name| name.eq_ignore_ascii_case(candidate))
    };
    if !collides(base) {
        return base.to_owned();
    }
    for suffix in 2.. {
        let ending = format!(" ({suffix})");
        let max_prefix = 64_usize.saturating_sub(ending.chars().count());
        let prefix = base.chars().take(max_prefix).collect::<String>();
        let candidate = format!("{prefix}{ending}");
        if !collides(&candidate) {
            return candidate;
        }
    }
    unreachable!("a free profile name must exist")
}
