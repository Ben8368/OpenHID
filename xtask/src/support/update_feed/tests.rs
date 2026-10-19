use super::{MacArch, appcast_url};

#[test]
fn each_architecture_reads_its_own_feed_on_the_channel() {
    assert_eq!(
        appcast_url("https://updates.example.org/", MacArch::Arm64),
        "https://updates.example.org/channels/stable/appcast-arm64.xml"
    );
    assert_eq!(
        appcast_url("https://mirror.example.org", MacArch::X86_64),
        "https://mirror.example.org/channels/stable/appcast-x86_64.xml"
    );
}

#[test]
fn release_dmg_names_carry_their_architecture() {
    for (name, arch) in [
        ("OpenHID-v0.8.14-macos-arm64.dmg", Some(MacArch::Arm64)),
        ("OpenHID-v0.8.14-macos-x86_64.dmg", Some(MacArch::X86_64)),
        ("OpenHID-v0.8.14-macos-universal.dmg", None),
        ("OpenHID-v0.8.14-macos-arm64.dmg.minisig", None),
        ("OpenHID-v0.8.14-windows-x86_64.msi", None),
        ("OpenHID.dmg", None),
    ] {
        assert_eq!(MacArch::of_dmg(name), arch, "{name}");
    }
}
