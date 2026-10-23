//! Verified AULA F87 PRO wireless control collection.

/// Standalone driver identity; this device does not speak Logitech HID++.
pub const DRIVER_ID: &str = "aula-f87-pro-wireless";
/// Driver identity for the verified F87 Pro wired Feature Report transport.
pub const WIRED_DRIVER_ID: &str = "aula-f87-pro-wired";

const F87_PRO_WIRELESS_PSD: [u8; 6] = [0x03, 0, 0, 0, 1, 0x0b];

/// Whether the six-byte wireless profile discriminator identifies an F87 Pro.
#[must_use]
pub fn matches_f87_pro_wireless_psd(psd: &[u8; 6]) -> bool {
    *psd == F87_PRO_WIRELESS_PSD
}

/// Whether this complete HID identity is the verified F87 PRO control interface.
///
/// The receiver's FF04 feature collection and standard input collections are
/// deliberately excluded. The shared wired VID/PID is not verified here.
#[must_use]
pub const fn matches_control(vendor: u16, product: u16, page: u16, usage: u16) -> bool {
    vendor == 0x3554 && product == 0xfa09 && page == 0xff02 && usage == 0x0002
}

/// Whether VID/PID is a candidate for the wired F87 Pro model query.
///
/// This identifies only a candidate transport. The device must answer the
/// `0x06/0x82` query with the F87 Pro model code before settings I/O is allowed.
#[must_use]
pub const fn matches_wired_candidate(vendor: u16, product: u16) -> bool {
    vendor == 0x258a && product == 0x010c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_only_the_verified_control_collection() {
        assert!(matches_control(0x3554, 0xfa09, 0xff02, 2));
        for (vendor, product, page, usage) in [
            (0x3554, 0xfa09, 0xff04, 2),
            (0x3554, 0xfa09, 1, 6),
            (0x3554, 0xfa09, 0xff02, 1),
            (0x258a, 0x010c, 0xff02, 2),
            (0x3554, 0xfa08, 0xff02, 2),
        ] {
            assert!(!matches_control(vendor, product, page, usage));
        }
    }
}
