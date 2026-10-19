use std::fs;

use super::*;

#[test]
fn msi_registration_must_identify_this_copy_and_wins_over_portable_marker() {
    let temp = tempfile::tempdir().unwrap();
    let installed = temp.path().join("Installed");
    let portable = temp.path().join("Portable");
    for directory in [&installed, &portable] {
        fs::create_dir(directory).unwrap();
        fs::write(directory.join("OpenHID.exe"), "binary").unwrap();
        fs::write(
            directory.join("openhid-installation.json"),
            r#"{"source":"windows-portable"}"#,
        )
        .unwrap();
    }
    assert_eq!(
        detect_in(&installed.join("OpenHID.exe"), Some(&installed)),
        InstallationSource::WindowsMsi
    );
    assert_eq!(
        detect_in(&portable.join("OpenHID.exe"), Some(&installed)),
        InstallationSource::WindowsPortable
    );
    fs::remove_file(portable.join("openhid-installation.json")).unwrap();
    assert_eq!(
        detect_in(&portable.join("OpenHID.exe"), Some(&installed)),
        InstallationSource::Unknown
    );
    // Failures to resolve either path must never compare as equal.
    assert_eq!(
        detect_in(
            &temp.path().join("Missing/OpenHID.exe"),
            Some(&temp.path().join("Missing"))
        ),
        InstallationSource::Unknown
    );
}

#[test]
fn portable_requires_the_exact_marker_next_to_the_running_gui() {
    let temp = tempfile::tempdir().unwrap();
    let executable = temp.path().join("OpenHID.exe");
    fs::write(&executable, "binary").unwrap();
    assert_eq!(detect_in(&executable, None), InstallationSource::Unknown);
    for marker in [
        "",
        "broken",
        "{}",
        r#"{"source":"msi"}"#,
        r#"{"source":"windows-portable-v2"}"#,
    ] {
        fs::write(temp.path().join("openhid-installation.json"), marker).unwrap();
        assert_eq!(detect_in(&executable, None), InstallationSource::Unknown);
    }
    fs::write(
        temp.path().join("openhid-installation.json"),
        r#"{"source":"windows-portable"}"#,
    )
    .unwrap();
    assert_eq!(
        detect_in(&executable, None),
        InstallationSource::WindowsPortable
    );
    let other = temp.path().join("openhid-agent.exe");
    fs::write(&other, "binary").unwrap();
    assert_eq!(detect_in(&other, None), InstallationSource::Unknown);
}

#[test]
fn packaging_supplies_the_portable_marker_and_path_bound_msi_receipt() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows");
    if !root.exists() {
        // Nix's source fileset deliberately excludes Windows packaging inputs.
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    fs::copy(
        root.join("openhid-installation.json"),
        temp.path().join("openhid-installation.json"),
    )
    .unwrap();
    let executable = temp.path().join("OpenHID.exe");
    fs::write(&executable, "binary").unwrap();
    assert_eq!(
        detect_in(&executable, None),
        InstallationSource::WindowsPortable
    );
    let wix = fs::read_to_string(root.join("OpenHID.wxs"))
        .unwrap()
        .replace("\r\n", "\n");
    assert!(wix.contains("Name=\"InstallLocation\"\n                       Type=\"string\"\n                       Value=\"[INSTALLFOLDER]\""));
    assert!(
        !wix.contains("openhid-installation.json"),
        "MSI must not install the portable marker"
    );
}
