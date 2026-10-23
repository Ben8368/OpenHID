//! Offline codec for the separately documented AULA wired keymap report.
//!
//! This report format is not the wireless `0x13/0x41` matrix protocol. The
//! module includes factory templates reconstructed from MIT-licensed F87 USB
//! captures, and still performs no device I/O.

use thiserror::Error;

use super::{
    AULA_F87_KEY_POSITIONS, AulaOemBaseLayer, InvalidAulaOemProfile, aula_keymap_action_outputs,
    aula_keymap_slot_is_assignable,
};

/// Feature Report ID used by the documented wired keymap upload format.
pub const AULA_WIRED_KEYMAP_REPORT_ID: u8 = 0x06;
/// Complete Feature Report buffer length, including the report ID byte.
pub const AULA_WIRED_KEYMAP_BLOB_LENGTH: usize = 520;

const HEADER_FIXED: [(usize, u8); 5] = [(0, 0x06), (1, 0x03), (3, 0x00), (4, 0x01), (5, 0x00)];
const LAYER_OFFSET: usize = 2;
const SLOT_BASE: usize = 0x08;
const SLOT_STRIDE: usize = 4;
const SLOT_USAGE_OFFSET: usize = 3;
const TRAILER: [u8; 2] = [0x5a, 0xa5];

/// Resolve a named single-key output in the 520-byte wired Feature Report format.
///
/// Wired slots store an AULA usage page and one 8-bit usage ID. Standalone HID
/// modifier usages are converted to their keyboard-page usage IDs. Modifier
/// chords, OEM actions and consumer usages above `0xff` need more fields, so
/// this encoder rejects them.
#[must_use]
pub fn aula_wired_keymap_action_output(name: &str) -> Option<[u8; 4]> {
    let normalized = name.trim().to_ascii_lowercase();
    aula_keymap_action_outputs()
        .into_iter()
        .find(|output| output.name() == normalized)
        .and_then(|output| wired_slot_from_matrix_action(output.raw()))
}

/// Return the named outputs representable by the wired Feature Report.
#[must_use]
pub fn aula_wired_keymap_action_outputs() -> Vec<(String, String, [u8; 4])> {
    aula_keymap_action_outputs()
        .into_iter()
        .filter_map(|output| {
            wired_slot_from_matrix_action(output.raw())
                .map(|raw| (output.name().to_owned(), output.label().to_owned(), raw))
        })
        .collect()
}

fn wired_slot_from_matrix_action(raw: [u8; 4]) -> Option<[u8; 4]> {
    match raw {
        [0, 0, 0, usage] => Some([0, 0, 0, usage]),
        [0x02, 0, 0, usage] if usage != 0 => Some([0x02, 0, 0, usage]),
        [0, modifier, 0, 0] => modifier_usage(modifier).map(|usage| [0, 0, 0, usage]),
        _ => None,
    }
}

/// Convert an editor matrix action to the wired report's single-usage slot.
///
/// OEM imports and editor actions must share this conversion so an action is
/// never classified as writable by a second, weaker rule.
#[must_use]
pub fn aula_wired_keymap_slot_from_matrix_action(raw: [u8; 4]) -> Option<[u8; 4]> {
    wired_slot_from_matrix_action(raw)
}

fn modifier_usage(modifier: u8) -> Option<u8> {
    // USB HID Keyboard-page modifier usages E0–E7 follow the report modifier-bit order.
    match modifier {
        0x01 => Some(0xe0),
        0x02 => Some(0xe1),
        0x04 => Some(0xe2),
        0x08 => Some(0xe3),
        0x10 => Some(0xe4),
        0x20 => Some(0xe5),
        0x40 => Some(0xe6),
        0x80 => Some(0xe7),
        _ => None,
    }
}

/// Number of complete four-byte slots that fit before the documented trailer.
///
/// The reference implementation computes a maximum slot index of 126 for its
/// 520-byte buffer. Its protocol prose says 128 slots, which would overlap the
/// trailer and exceed that length; do not treat incomplete trailing bytes as a
/// slot.
pub const AULA_WIRED_KEYMAP_SLOT_COUNT: usize =
    ((AULA_WIRED_KEYMAP_BLOB_LENGTH - TRAILER.len() - SLOT_BASE - SLOT_USAGE_OFFSET - 1)
        / SLOT_STRIDE)
        + 1;

/// Base or Fn layer selected by byte 2 of a wired keymap blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AulaWiredKeymapLayer {
    /// Base layer (`0x00`).
    Base,
    /// Fn layer (`0x01`).
    Fn,
}

impl AulaWiredKeymapLayer {
    const fn byte(self) -> u8 {
        match self {
            Self::Base => 0x00,
            Self::Fn => 0x01,
        }
    }

    const fn from_byte(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Base),
            0x01 => Some(Self::Fn),
            _ => None,
        }
    }
}

/// Validated raw wired keymap Feature Report with opaque bytes preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaWiredKeymapBlob {
    bytes: Vec<u8>,
    layer: AulaWiredKeymapLayer,
}

/// Return the packaged F87 wired factory template for the requested layer.
///
/// The templates come from MIT-licensed F87 USB captures. The Fn template is
/// unbound; using it as the starting point intentionally clears user Fn binds
/// when uploaded. Keep an original report copy before editing and writing.
pub fn aula_wired_keymap_factory_template(
    layer: AulaWiredKeymapLayer,
) -> Result<AulaWiredKeymapBlob, InvalidAulaWiredKeymapBlob> {
    let bytes: &[u8; AULA_WIRED_KEYMAP_BLOB_LENGTH] = match layer {
        AulaWiredKeymapLayer::Base => {
            include_bytes!("../../../assets/aula/wired-keymap-base.bin")
        }
        AulaWiredKeymapLayer::Fn => include_bytes!("../../../assets/aula/wired-keymap-fn.bin"),
    };
    AulaWiredKeymapBlob::from_bytes(bytes.to_vec())
}

/// Import the installed F87 PRO base assignments into a wired keymap report.
///
/// The function changes only assignable physical-key slots. OEM-reserved F1–F12
/// slots and all other template bytes remain untouched. Actions that do not
/// fit the single-usage wired format fail the conversion instead of being
/// dropped. The OEM Fn layer is intentionally separate and is not imported.
pub fn aula_wired_keymap_base_from_oem_profile(
    profile: &[u8],
) -> Result<AulaWiredKeymapBlob, InvalidAulaWiredKeymapImport> {
    let base = AulaOemBaseLayer::from_profile_dct(profile)?;
    let mut blob = aula_wired_keymap_factory_template(AulaWiredKeymapLayer::Base)?;
    for position in AULA_F87_KEY_POSITIONS {
        let slot = usize::from(position.matrix_slot());
        if !aula_keymap_slot_is_assignable(slot, false) {
            continue;
        }
        let action = base.entries()[slot];
        let wired = wired_slot_from_matrix_action(action)
            .ok_or(InvalidAulaWiredKeymapImport::UnrepresentableAction { slot, action })?;
        blob.replace_slot(slot, wired)?;
    }
    Ok(blob)
}

impl AulaWiredKeymapBlob {
    /// Validate and retain an existing 520-byte blob as the edit template.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, InvalidAulaWiredKeymapBlob> {
        if bytes.len() != AULA_WIRED_KEYMAP_BLOB_LENGTH {
            return Err(InvalidAulaWiredKeymapBlob::Length);
        }
        for (offset, expected) in HEADER_FIXED {
            if bytes[offset] != expected {
                return Err(InvalidAulaWiredKeymapBlob::Header);
            }
        }
        let layer = AulaWiredKeymapLayer::from_byte(bytes[LAYER_OFFSET])
            .ok_or(InvalidAulaWiredKeymapBlob::Layer)?;
        if bytes[AULA_WIRED_KEYMAP_BLOB_LENGTH - TRAILER.len()..] != TRAILER {
            return Err(InvalidAulaWiredKeymapBlob::Trailer);
        }
        Ok(Self { bytes, layer })
    }

    /// Layer encoded in the retained blob.
    #[must_use]
    pub const fn layer(&self) -> AulaWiredKeymapLayer {
        self.layer
    }

    /// Change the selected layer while preserving every other byte.
    pub fn set_layer(&mut self, layer: AulaWiredKeymapLayer) {
        self.bytes[LAYER_OFFSET] = layer.byte();
        self.layer = layer;
    }

    /// Return one raw four-byte slot by matrix/LED index.
    #[must_use]
    pub fn slot(&self, index: usize) -> Option<[u8; SLOT_STRIDE]> {
        let start = slot_start(index)?;
        let mut slot = [0; SLOT_STRIDE];
        slot.copy_from_slice(self.bytes.get(start..start + SLOT_STRIDE)?);
        Some(slot)
    }

    /// Replace one raw four-byte slot, retaining all other template bytes.
    pub fn replace_slot(
        &mut self,
        index: usize,
        slot: [u8; SLOT_STRIDE],
    ) -> Result<(), InvalidAulaWiredKeymapBlob> {
        let start = slot_start(index).ok_or(InvalidAulaWiredKeymapBlob::SlotIndex)?;
        let target = self
            .bytes
            .get_mut(start..start + SLOT_STRIDE)
            .ok_or(InvalidAulaWiredKeymapBlob::SlotIndex)?;
        target.copy_from_slice(&slot);
        Ok(())
    }

    /// Borrow the complete report bytes, including report ID and trailer.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume the blob and return its complete report bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

fn slot_start(index: usize) -> Option<usize> {
    if index >= AULA_WIRED_KEYMAP_SLOT_COUNT {
        return None;
    }
    SLOT_BASE.checked_add(index.checked_mul(SLOT_STRIDE)?)
}

/// A raw wired keymap report failed length, header, layer, trailer, or slot validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaWiredKeymapBlob {
    /// The report must be exactly 520 bytes.
    #[error("an AULA wired keymap blob must be exactly 520 bytes")]
    Length,
    /// One of the fixed report header bytes did not match the documented format.
    #[error("the AULA wired keymap blob header is invalid")]
    Header,
    /// The report selected an unknown keymap layer.
    #[error("the AULA wired keymap blob layer must be 0 or 1")]
    Layer,
    /// The final two bytes did not match the documented trailer.
    #[error("the AULA wired keymap blob trailer is invalid")]
    Trailer,
    /// The requested index does not address a complete four-byte slot.
    #[error("the AULA wired keymap slot index is out of range")]
    SlotIndex,
}

/// An OEM F87 base profile contains an action the wired keymap cannot express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaWiredKeymapImport {
    /// The source is not a valid installed F87 PRO OEM profile.
    #[error(transparent)]
    OemProfile(#[from] InvalidAulaOemProfile),
    /// The packaged wired factory template is invalid.
    #[error(transparent)]
    Template(#[from] InvalidAulaWiredKeymapBlob),
    /// The OEM source action cannot be represented by a single wired usage.
    #[error(
        "OEM base action {action:02x?} at F87 key slot {slot} is not representable in the wired report"
    )]
    UnrepresentableAction {
        /// Matrix/LED slot whose OEM action could not be converted.
        slot: usize,
        /// Four raw bytes decoded from the OEM base profile.
        action: [u8; 4],
    },
}
