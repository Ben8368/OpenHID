//! Shared, host-free F87 PRO lighting commands and readback state.

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod effects;
mod keymap_actions;
mod layout;
mod macro_file;
mod macro_transfer;
mod macro_tree;
mod oem_profile;
mod wired_config;
mod wired_keymap;
pub use effects::{AulaEffectCapabilities, aula_effect_capabilities};
pub use keymap_actions::{
    AulaKeymapActionOutput, aula_keymap_action_label, aula_keymap_action_output,
    aula_keymap_action_outputs,
};
pub use layout::{AULA_F87_KEY_POSITIONS, AulaKeyPosition};
pub use macro_file::{
    AULA_OEM_MACRO_PROFILE_FORMAT, AULA_OEM_MACRO_RECORD_LIMIT, AulaOemMacroEvent,
    AulaOemMacroEventKind, AulaOemMacroFile, AulaOemMacroProfile, InvalidAulaOemMacroFile,
    InvalidAulaOemMacroProfile, InvalidAulaOemMacroRecord,
};
pub use macro_transfer::{
    AulaOemMacroTransferEntry, AulaOemMacroTransferTable, InvalidAulaOemMacroTransferBlock,
    InvalidAulaOemMacroTransferTable, aula_oem_macro_mapped_virtual_keys,
    aula_oem_macro_transfer_block, aula_oem_macro_transfer_table,
};
pub use macro_tree::{
    AULA_OEM_MACRO_TREE_PROFILE_FORMAT, AULA_OEM_MACRO_TREE_ROOT_NODE_ID, AulaOemMacroTree,
    AulaOemMacroTreeNode, AulaOemMacroTreeProfile, InvalidAulaOemMacroTree,
    InvalidAulaOemMacroTreeProfile,
};
pub use oem_profile::{
    AULA_OEM_KEY_COMBINATION_ACTION, AulaOemBaseLayer, AulaOemFnActions, AulaOemFnLayer,
    AulaOemFnRecords, AulaOemKeyCombination, InvalidAulaOemProfile,
    aula_f87_oem_default_fn_actions, aula_f87_oem_default_fn_layer, write_keymap_to_profile_dct,
    write_keymap_to_profile_dct_with_fn_actions,
};
pub use wired_config::{
    AULA_WIRED_CONFIG_BLOCK_BYTE_LENGTH, AULA_WIRED_CONFIG_BLOCK_LENGTH, AulaWiredConfigBlock,
    InvalidAulaWiredConfigBlock, InvalidAulaWiredConfigChange,
};
pub use wired_keymap::{
    AULA_WIRED_KEYMAP_BLOB_LENGTH, AULA_WIRED_KEYMAP_REPORT_ID, AULA_WIRED_KEYMAP_SLOT_COUNT,
    AulaWiredKeymapBlob, AulaWiredKeymapLayer, InvalidAulaWiredKeymapBlob,
    InvalidAulaWiredKeymapImport, aula_wired_keymap_action_output,
    aula_wired_keymap_action_outputs, aula_wired_keymap_base_from_oem_profile,
    aula_wired_keymap_factory_template, aula_wired_keymap_slot_from_matrix_action,
};

/// Number of remappable physical positions in the F87 PRO base layer.
pub const AULA_BASE_KEY_COUNT: usize = 126;
/// Number of four-byte slots in the F87 PRO Fn layer.
pub const AULA_FN_KEY_COUNT: usize = 126;
/// Number of Fn records reserved by the installed OEM `profile.dct` format.
///
/// The local file format reserves 128 rows, while the wireless matrix returned
/// by the keyboard contains 126 four-byte entries.
pub const AULA_OEM_FN_RECORD_COUNT: usize = 128;

const KEYMAP_DISABLE_ACTION_RAW: [u8; 4] = [0xa2, 0, 0, 0x08];
const KEYMAP_ALT_ESCAPE_ACTION_RAW: [u8; 4] = [0x29, 0, 0x04, 0];
const KEYMAP_ALT_TAB_ACTION_RAW: [u8; 4] = [0x2b, 0, 0x04, 0];
const KEYMAP_FN_LAYER_ACTION_RAW: [u8; 4] = [0x0d, 0, 0, 0];

/// F87 PRO keymap actions confirmed in the installed OEM Fn profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AulaKeymapAction {
    /// OEM action that disables the selected key.
    DisableKey,
    /// Alt+Escape.
    AltEscape,
    /// Alt+Tab.
    AltTab,
}

impl AulaKeymapAction {
    /// Decode a confirmed F87 PRO action while retaining unknown OEM bytes elsewhere.
    #[must_use]
    pub const fn from_raw(raw: [u8; 4]) -> Option<Self> {
        match raw {
            KEYMAP_DISABLE_ACTION_RAW => Some(Self::DisableKey),
            KEYMAP_ALT_ESCAPE_ACTION_RAW => Some(Self::AltEscape),
            KEYMAP_ALT_TAB_ACTION_RAW => Some(Self::AltTab),
            _ => None,
        }
    }

    /// Raw four-byte matrix representation used by the wireless Fn table.
    #[must_use]
    pub const fn raw(self) -> [u8; 4] {
        match self {
            Self::DisableKey => KEYMAP_DISABLE_ACTION_RAW,
            Self::AltEscape => KEYMAP_ALT_ESCAPE_ACTION_RAW,
            Self::AltTab => KEYMAP_ALT_TAB_ACTION_RAW,
        }
    }

    /// Stable keyboard label suitable for diagnostics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DisableKey => "Disable key (OEM)",
            Self::AltEscape => "Alt+Esc",
            Self::AltTab => "Alt+Tab",
        }
    }
}

/// Validated raw base-layer entries from the F87 PRO wireless matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedAulaBaseMatrix", into = "UncheckedAulaBaseMatrix")]
pub struct AulaBaseMatrix {
    entries: Vec<[u8; 4]>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedAulaBaseMatrix {
    entries: Vec<[u8; 4]>,
}

impl AulaBaseMatrix {
    /// Construct a base matrix from exactly 126 four-byte physical-key entries.
    pub fn new(entries: Vec<[u8; 4]>) -> Result<Self, InvalidAulaBaseMatrix> {
        if entries.len() != AULA_BASE_KEY_COUNT {
            return Err(InvalidAulaBaseMatrix);
        }
        Ok(Self { entries })
    }

    /// Base-layer entries in device matrix-index order.
    #[must_use]
    pub fn entries(&self) -> &[[u8; 4]] {
        &self.entries
    }
}

impl TryFrom<UncheckedAulaBaseMatrix> for AulaBaseMatrix {
    type Error = InvalidAulaBaseMatrix;

    fn try_from(value: UncheckedAulaBaseMatrix) -> Result<Self, Self::Error> {
        Self::new(value.entries)
    }
}

impl From<AulaBaseMatrix> for UncheckedAulaBaseMatrix {
    fn from(matrix: AulaBaseMatrix) -> Self {
        Self {
            entries: matrix.entries,
        }
    }
}

/// A wireless base matrix does not contain exactly 126 entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("an AULA base matrix requires exactly 126 entries")]
pub struct InvalidAulaBaseMatrix;

// Captured F87 OEM assignments reserve F1–F12 as keyboard function sources.
const AULA_RESERVED_KEYMAP_SLOTS: [u8; 12] = [12, 18, 24, 30, 36, 42, 48, 54, 60, 66, 72, 78];

/// Whether an F87 keymap slot is available as an assignment source.
#[must_use]
pub const fn aula_keymap_slot_is_assignable(slot: usize, fn_layer: bool) -> bool {
    let layer_len = if fn_layer {
        AULA_FN_KEY_COUNT
    } else {
        AULA_BASE_KEY_COUNT
    };
    if slot >= layer_len {
        return false;
    }
    if fn_layer {
        return true;
    }
    let mut index = 0;
    while index < AULA_RESERVED_KEYMAP_SLOTS.len() {
        if AULA_RESERVED_KEYMAP_SLOTS[index] as usize == slot {
            return false;
        }
        index += 1;
    }
    true
}

/// Complete raw base and Fn mappings. Four-byte entries retain unknown OEM actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedAulaKeymap", into = "UncheckedAulaKeymap")]
pub struct AulaKeymap {
    base: Vec<[u8; 4]>,
    fn_layer: Vec<[u8; 4]>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedAulaKeymap {
    base: Vec<[u8; 4]>,
    fn_layer: Vec<[u8; 4]>,
}

impl AulaKeymap {
    /// Construct a complete keymap from raw four-byte matrix entries.
    pub fn new(base: Vec<[u8; 4]>, fn_layer: Vec<[u8; 4]>) -> Result<Self, InvalidAulaKeymap> {
        if base.len() != AULA_BASE_KEY_COUNT || fn_layer.len() != AULA_FN_KEY_COUNT {
            return Err(InvalidAulaKeymap);
        }
        Ok(Self { base, fn_layer })
    }

    /// Base-layer mappings in device matrix-index order.
    #[must_use]
    pub fn base(&self) -> &[[u8; 4]] {
        &self.base
    }

    /// Fn-layer mappings in device matrix-index order.
    #[must_use]
    pub fn fn_layer(&self) -> &[[u8; 4]] {
        &self.fn_layer
    }

    /// Find the first OEM-reserved F1–F12 base slot changed from a device snapshot.
    #[must_use]
    pub fn first_reserved_slot_changed(&self, original: &Self) -> Option<u8> {
        AULA_RESERVED_KEYMAP_SLOTS.into_iter().find(|slot| {
            let index = usize::from(*slot);
            self.base[index] != original.base[index]
        })
    }
}

impl TryFrom<UncheckedAulaKeymap> for AulaKeymap {
    type Error = InvalidAulaKeymap;

    fn try_from(value: UncheckedAulaKeymap) -> Result<Self, Self::Error> {
        Self::new(value.base, value.fn_layer)
    }
}

impl From<AulaKeymap> for UncheckedAulaKeymap {
    fn from(keymap: AulaKeymap) -> Self {
        Self {
            base: keymap.base,
            fn_layer: keymap.fn_layer,
        }
    }
}

/// A keymap backup does not contain exactly 126 base and 126 Fn entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("an AULA keymap requires exactly 126 base and 126 Fn entries")]
pub struct InvalidAulaKeymap;

/// Stable format identifier for portable AULA wireless keymap profiles.
pub const AULA_KEYMAP_PROFILE_FORMAT: &str = "openhid-aula-wireless-keymap";

/// A versioned portable base+Fn snapshot shared by the desktop and CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "UncheckedAulaKeymapProfile",
    into = "UncheckedAulaKeymapProfile"
)]
pub struct AulaKeymapProfile {
    keymap: AulaKeymap,
    oem_fn_records: Option<AulaOemFnRecords>,
    oem_fn_actions: Option<AulaOemFnActions>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedAulaKeymapProfile {
    format: String,
    version: u8,
    base: Vec<[u8; 4]>,
    fn_layer: Vec<[u8; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oem_fn_records: Option<AulaOemFnRecords>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    oem_fn_actions: Option<AulaOemFnActions>,
}

impl AulaKeymapProfile {
    /// Current portable JSON schema version.
    pub const VERSION: u8 = 4;

    /// Wrap a complete validated keymap in the shared portable format.
    #[must_use]
    pub const fn new(keymap: AulaKeymap) -> Self {
        Self {
            keymap,
            oem_fn_records: None,
            oem_fn_actions: None,
        }
    }

    /// Wrap a keymap together with opaque fields retained from an OEM profile.
    #[must_use]
    pub const fn with_oem_fn_records(
        keymap: AulaKeymap,
        oem_fn_records: Option<AulaOemFnRecords>,
    ) -> Self {
        Self {
            keymap,
            oem_fn_records,
            oem_fn_actions: None,
        }
    }

    /// Wrap a keymap with both opaque OEM row fields and original Fn actions.
    #[must_use]
    pub const fn with_oem_fn_data(
        keymap: AulaKeymap,
        oem_fn_records: Option<AulaOemFnRecords>,
        oem_fn_actions: Option<AulaOemFnActions>,
    ) -> Self {
        Self {
            keymap,
            oem_fn_records,
            oem_fn_actions,
        }
    }

    /// The validated base and Fn mappings carried by this profile.
    #[must_use]
    pub const fn keymap(&self) -> &AulaKeymap {
        &self.keymap
    }

    /// Opaque OEM Fn record fields, when this profile came from an OEM file.
    #[must_use]
    pub const fn oem_fn_records(&self) -> Option<&AulaOemFnRecords> {
        self.oem_fn_records.as_ref()
    }

    /// Original OEM Fn actions retained for lossless local-profile export.
    #[must_use]
    pub const fn oem_fn_actions(&self) -> Option<&AulaOemFnActions> {
        self.oem_fn_actions.as_ref()
    }
}

impl TryFrom<UncheckedAulaKeymapProfile> for AulaKeymapProfile {
    type Error = InvalidAulaKeymapProfile;

    fn try_from(profile: UncheckedAulaKeymapProfile) -> Result<Self, Self::Error> {
        if profile.format != AULA_KEYMAP_PROFILE_FORMAT {
            return Err(InvalidAulaKeymapProfile::Format);
        }
        if !matches!(profile.version, 1..=Self::VERSION) {
            return Err(InvalidAulaKeymapProfile::Version);
        }
        let fn_layer = if profile.version < Self::VERSION {
            migrate_legacy_fn_layer(profile.fn_layer)?
        } else {
            profile.fn_layer
        };
        let keymap =
            AulaKeymap::new(profile.base, fn_layer).map_err(InvalidAulaKeymapProfile::Keymap)?;
        Ok(Self::with_oem_fn_data(
            keymap,
            profile.oem_fn_records,
            profile.oem_fn_actions,
        ))
    }
}

fn migrate_legacy_fn_layer(
    mut fn_layer: Vec<[u8; 4]>,
) -> Result<Vec<[u8; 4]>, InvalidAulaKeymapProfile> {
    if fn_layer.len() != AULA_OEM_FN_RECORD_COUNT {
        return Err(InvalidAulaKeymapProfile::Keymap(InvalidAulaKeymap));
    }
    if fn_layer[AULA_FN_KEY_COUNT..]
        .iter()
        .any(|entry| *entry != [0; 4])
    {
        return Err(InvalidAulaKeymapProfile::UnmappedLegacyFnEntries);
    }
    fn_layer.truncate(AULA_FN_KEY_COUNT);
    Ok(fn_layer)
}

impl From<AulaKeymapProfile> for UncheckedAulaKeymapProfile {
    fn from(profile: AulaKeymapProfile) -> Self {
        Self {
            format: AULA_KEYMAP_PROFILE_FORMAT.to_owned(),
            version: AulaKeymapProfile::VERSION,
            base: profile.keymap.base,
            fn_layer: profile.keymap.fn_layer,
            oem_fn_records: profile.oem_fn_records,
            oem_fn_actions: profile.oem_fn_actions,
        }
    }
}

/// A portable AULA keymap profile has the wrong version, identifier, or table lengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaKeymapProfile {
    /// The JSON document uses a different profile format.
    #[error("invalid AULA keymap profile format")]
    Format,
    /// The JSON document uses an unsupported schema version.
    #[error("unsupported AULA keymap profile version")]
    Version,
    /// A legacy Fn profile uses the two matrix slots absent from the wireless device table.
    #[error("legacy AULA keymap uses Fn slots 126–127, which do not exist in the wireless matrix")]
    UnmappedLegacyFnEntries,
    /// The base or Fn table is incomplete.
    #[error(transparent)]
    Keymap(#[from] InvalidAulaKeymap),
}

/// Current lighting fields; unknown/custom effect identifiers are retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightingState {
    /// Native effect identifier.
    pub effect: u8,
    /// Brightness when the current effect has a known parameter table.
    pub brightness: Option<u8>,
    /// Speed when the current effect has a known parameter table.
    pub speed: Option<u8>,
    /// Current color mode encoded in the effect parameter table, when known.
    pub color_mode: Option<AulaColorModeState>,
}

/// Current F87 PRO battery percentage and the receiver's power-source marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AulaBattery {
    /// Percentage-based battery presentation shared with other device families.
    pub info: crate::device::BatteryInfo,
    /// Power source reported by the F87 PRO receiver.
    pub power_source: AulaBatteryPowerSource,
}

/// Power-source values reported by the F87 PRO wireless battery command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AulaBatteryPowerSource {
    /// The keyboard reports that it is operating from its battery.
    Battery,
    /// The keyboard reports cable power; active charging is not distinguished.
    Cable,
}

/// Color behavior decoded from the current wireless effect table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AulaColorModeState {
    /// The device's standard single-color palette mode.
    SingleColor,
    /// The effect cycles through its built-in colors.
    Colorful,
    /// An unrecognized palette selector retained from the device.
    Palette(u8),
}

/// A supported color-mode edit for an AULA built-in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AulaColorModeChange {
    /// Use an explicit RGB color for effects that support custom color.
    SingleColor(AulaRgb),
    /// Enable the effect's color-cycling mode.
    Colorful,
}

/// Decoded settings exposed by the F87 PRO wireless control collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AulaState {
    /// Built-in lighting controls.
    pub lighting: LightingState,
    /// Wireless idle timeout, when its device value is recognized.
    pub sleep_timeout: Option<SleepTimeout>,
    /// Wireless key debounce, when its device value is recognized.
    pub debounce: Option<AulaDebounce>,
    /// Side-light effect, when its configuration value is recognized.
    pub side_light_effect: Option<AulaSideLightEffect>,
    /// Current battery reading from the F87 PRO wireless receiver, when available.
    pub battery: Option<AulaBattery>,
}

impl AulaState {
    /// Construct a readback state from independently decoded settings.
    #[must_use]
    pub const fn new(lighting: LightingState, sleep_timeout: Option<SleepTimeout>) -> Self {
        Self {
            lighting,
            sleep_timeout,
            debounce: None,
            side_light_effect: None,
            battery: None,
        }
    }

    /// Attach a decoded debounce setting to this readback state.
    #[must_use]
    pub const fn with_debounce(mut self, debounce: Option<AulaDebounce>) -> Self {
        self.debounce = debounce;
        self
    }

    /// Attach the decoded side-light effect to this readback state.
    #[must_use]
    pub const fn with_side_light_effect(
        mut self,
        side_light_effect: Option<AulaSideLightEffect>,
    ) -> Self {
        self.side_light_effect = side_light_effect;
        self
    }

    /// Attach the optional battery reading to this settings snapshot.
    #[must_use]
    pub fn with_battery(mut self, battery: Option<AulaBattery>) -> Self {
        self.battery = battery;
        self
    }
}

/// A verified F87 PRO side-light effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum AulaSideLightEffect {
    /// Turn the side lights off.
    Off,
    /// Move a rainbow along the side lights.
    RainbowWave,
    /// Show the rainbow colors together.
    RainbowTogether,
    /// Show one solid side-light color.
    Solid,
    /// Fade the side lights in and out.
    Breathing,
}

const AULA_SIDE_LIGHT_EFFECTS: [AulaSideLightEffect; 5] = [
    AulaSideLightEffect::Off,
    AulaSideLightEffect::RainbowWave,
    AulaSideLightEffect::RainbowTogether,
    AulaSideLightEffect::Solid,
    AulaSideLightEffect::Breathing,
];

impl AulaSideLightEffect {
    /// Supported side-light effects in device order.
    #[must_use]
    pub const fn options() -> &'static [Self; 5] {
        &AULA_SIDE_LIGHT_EFFECTS
    }

    /// Decode a configuration value, preserving unknown values in the raw snapshot.
    #[must_use]
    pub const fn from_wire(value: u8) -> Option<Self> {
        let mut index = 0;
        while index < AULA_SIDE_LIGHT_EFFECTS.len() {
            let effect = AULA_SIDE_LIGHT_EFFECTS[index];
            if effect.wire_value() == value {
                return Some(effect);
            }
            index += 1;
        }
        None
    }

    /// Encoded configuration value.
    #[must_use]
    pub const fn wire_value(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::RainbowWave => 1,
            Self::RainbowTogether => 2,
            Self::Solid => 3,
            Self::Breathing => 4,
        }
    }
}

impl TryFrom<u8> for AulaSideLightEffect {
    type Error = InvalidAulaSideLightEffect;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_wire(value).ok_or(InvalidAulaSideLightEffect)
    }
}

impl From<AulaSideLightEffect> for u8 {
    fn from(value: AulaSideLightEffect) -> Self {
        value.wire_value()
    }
}

/// A side-light effect value outside the verified F87 PRO range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported AULA side-light effect; choose one of the advertised values")]
pub struct InvalidAulaSideLightEffect;

/// A supported wireless key debounce interval in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct AulaDebounce(u8);

const AULA_DEBOUNCE_OPTIONS: [AulaDebounce; 5] = [
    AulaDebounce(1),
    AulaDebounce(2),
    AulaDebounce(3),
    AulaDebounce(4),
    AulaDebounce(5),
];

impl AulaDebounce {
    /// Supported debounce intervals in ascending order.
    #[must_use]
    pub const fn options() -> &'static [Self; 5] {
        &AULA_DEBOUNCE_OPTIONS
    }

    /// Construct from the user-facing interval in milliseconds.
    #[must_use]
    pub fn from_milliseconds(milliseconds: u8) -> Option<Self> {
        Self::options()
            .iter()
            .copied()
            .find(|value| value.milliseconds() == milliseconds)
    }

    /// Decode the observed device value; unknown values remain in the raw snapshot.
    #[must_use]
    pub fn from_wire(value: u8) -> Option<Self> {
        Self::options()
            .iter()
            .copied()
            .find(|debounce| debounce.wire_value() == value)
    }

    /// User-facing debounce interval in milliseconds.
    #[must_use]
    pub const fn milliseconds(self) -> u8 {
        self.0
    }

    /// Encoded device value.
    #[must_use]
    pub const fn wire_value(self) -> u8 {
        self.0 - 1
    }
}

impl TryFrom<u8> for AulaDebounce {
    type Error = InvalidAulaDebounce;

    fn try_from(milliseconds: u8) -> Result<Self, Self::Error> {
        Self::from_milliseconds(milliseconds).ok_or(InvalidAulaDebounce)
    }
}

impl From<AulaDebounce> for u8 {
    fn from(value: AulaDebounce) -> Self {
        value.milliseconds()
    }
}

/// A debounce interval outside the keyboard's supported range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported AULA debounce interval; choose 1–5 ms")]
pub struct InvalidAulaDebounce;

/// A supported wireless idle timeout, represented in seconds at API boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct SleepTimeout(u8);

const SLEEP_TIMEOUTS: [SleepTimeout; 9] = [
    SleepTimeout(2),
    SleepTimeout(3),
    SleepTimeout(4),
    SleepTimeout(6),
    SleepTimeout(8),
    SleepTimeout(10),
    SleepTimeout(20),
    SleepTimeout(30),
    SleepTimeout(40),
];

impl SleepTimeout {
    /// All supported idle timeouts in ascending order.
    #[must_use]
    pub const fn options() -> &'static [Self; 9] {
        &SLEEP_TIMEOUTS
    }

    /// Construct a timeout from the user-facing duration in seconds.
    #[must_use]
    pub fn from_seconds(seconds: u16) -> Option<Self> {
        Self::options()
            .iter()
            .copied()
            .find(|timeout| timeout.seconds() == seconds)
    }

    /// Decode a device value, leaving unknown values untouched in the snapshot.
    #[must_use]
    pub fn from_wire(value: u8) -> Option<Self> {
        Self::options()
            .iter()
            .copied()
            .find(|timeout| timeout.0 == value)
    }

    /// User-facing duration in seconds.
    #[must_use]
    pub const fn seconds(self) -> u16 {
        (self.0 as u16) * 30
    }

    /// Encoded value used by the verified F87 PRO configuration block.
    #[must_use]
    pub const fn wire_value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u16> for SleepTimeout {
    type Error = InvalidSleepTimeout;

    fn try_from(seconds: u16) -> Result<Self, Self::Error> {
        Self::from_seconds(seconds).ok_or(InvalidSleepTimeout)
    }
}

impl From<SleepTimeout> for u16 {
    fn from(timeout: SleepTimeout) -> Self {
        timeout.seconds()
    }
}

/// A timeout value outside the keyboard's supported wireless options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported AULA sleep timeout; choose one of the advertised values")]
pub struct InvalidSleepTimeout;

/// A validated setting edit. Unspecified fields and unknown bytes are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedAulaChange", into = "UncheckedAulaChange")]
pub struct AulaChange {
    lighting: Option<LightingChange>,
    sleep_timeout: Option<SleepTimeout>,
    debounce: Option<AulaDebounce>,
    side_light_effect: Option<AulaSideLightEffect>,
}

#[derive(Serialize, Deserialize)]
struct UncheckedAulaChange {
    lighting: Option<LightingChange>,
    sleep_timeout: Option<SleepTimeout>,
    debounce: Option<AulaDebounce>,
    side_light_effect: Option<AulaSideLightEffect>,
}

impl AulaChange {
    /// Construct a non-empty edit to one or more independently supported fields.
    pub fn new(
        lighting: Option<LightingChange>,
        sleep_timeout: Option<SleepTimeout>,
    ) -> Result<Self, InvalidAulaChange> {
        Self::new_with_debounce(lighting, sleep_timeout, None)
    }

    /// Construct a non-empty edit to lighting, sleep, or debounce settings.
    pub fn new_with_debounce(
        lighting: Option<LightingChange>,
        sleep_timeout: Option<SleepTimeout>,
        debounce: Option<AulaDebounce>,
    ) -> Result<Self, InvalidAulaChange> {
        Self::new_with_side_light(lighting, sleep_timeout, debounce, None)
    }

    /// Construct a non-empty edit including the verified side-light effect.
    pub fn new_with_side_light(
        lighting: Option<LightingChange>,
        sleep_timeout: Option<SleepTimeout>,
        debounce: Option<AulaDebounce>,
        side_light_effect: Option<AulaSideLightEffect>,
    ) -> Result<Self, InvalidAulaChange> {
        if lighting.is_none()
            && sleep_timeout.is_none()
            && debounce.is_none()
            && side_light_effect.is_none()
        {
            return Err(InvalidAulaChange);
        }
        Ok(Self {
            lighting,
            sleep_timeout,
            debounce,
            side_light_effect,
        })
    }

    /// Lighting fields to change, if selected.
    #[must_use]
    pub const fn lighting(self) -> Option<LightingChange> {
        self.lighting
    }

    /// Sleep timeout to change, if selected.
    #[must_use]
    pub const fn sleep_timeout(self) -> Option<SleepTimeout> {
        self.sleep_timeout
    }

    /// Debounce interval to change, if selected.
    #[must_use]
    pub const fn debounce(self) -> Option<AulaDebounce> {
        self.debounce
    }

    /// Side-light effect to change, if selected.
    #[must_use]
    pub const fn side_light_effect(self) -> Option<AulaSideLightEffect> {
        self.side_light_effect
    }
}

impl TryFrom<UncheckedAulaChange> for AulaChange {
    type Error = InvalidAulaChange;

    fn try_from(value: UncheckedAulaChange) -> Result<Self, Self::Error> {
        Self::new_with_side_light(
            value.lighting,
            value.sleep_timeout,
            value.debounce,
            value.side_light_effect,
        )
    }
}

impl From<AulaChange> for UncheckedAulaChange {
    fn from(value: AulaChange) -> Self {
        Self {
            lighting: value.lighting,
            sleep_timeout: value.sleep_timeout,
            debounce: value.debounce,
            side_light_effect: value.side_light_effect,
        }
    }
}

/// A rejected empty AULA settings edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("an AULA settings edit must change at least one field")]
pub struct InvalidAulaChange;

/// Resolve only the verified wireless control collection's opaque identity.
/// Other raw collections and HID++ routes must never receive AULA commands.
#[must_use]
pub fn route_identity(route: &super::DeviceRoute) -> Option<&str> {
    match route {
        super::DeviceRoute::RawHid {
            vendor_id,
            product_id,
            usage_page,
            usage_id,
            identity,
        } if openhid_device_registry::aula::matches_control(
            *vendor_id,
            *product_id,
            *usage_page,
            *usage_id,
        ) =>
        {
            Some(identity)
        }
        _ => None,
    }
}

/// Resolve only a raw route whose VID/PID can carry the wired F87 Pro query.
///
/// The HID collection must still pass the read-only model probe before any
/// configuration is read or written.
#[must_use]
pub fn wired_route_identity(route: &super::DeviceRoute) -> Option<&str> {
    match route {
        super::DeviceRoute::RawHid {
            vendor_id,
            product_id,
            identity,
            ..
        } if openhid_device_registry::aula::matches_wired_candidate(*vendor_id, *product_id) => {
            Some(identity)
        }
        _ => None,
    }
}

/// A rejected edit, including empty edits and parameters on the off effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported AULA lighting edit (effects 0..18, levels 0..4)")]
pub struct InvalidLightingChange;

/// Validated edit; unspecified fields and unknown configuration bytes survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedChange", into = "UncheckedChange")]
pub struct LightingChange {
    effect: Option<u8>,
    brightness: Option<u8>,
    speed: Option<u8>,
    color_mode: Option<AulaColorModeChange>,
}

#[derive(Serialize, Deserialize)]
struct UncheckedChange {
    effect: Option<u8>,
    brightness: Option<u8>,
    speed: Option<u8>,
    color_mode: Option<AulaColorModeChange>,
}

impl TryFrom<UncheckedChange> for LightingChange {
    type Error = InvalidLightingChange;
    fn try_from(value: UncheckedChange) -> Result<Self, Self::Error> {
        let change = Self::new(value.effect, value.brightness, value.speed)?;
        if let Some(color_mode) = value.color_mode {
            change.with_color_mode(color_mode)
        } else {
            Ok(change)
        }
    }
}

impl From<LightingChange> for UncheckedChange {
    fn from(value: LightingChange) -> Self {
        Self {
            effect: value.effect,
            brightness: value.brightness,
            speed: value.speed,
            color_mode: value.color_mode,
        }
    }
}

impl LightingChange {
    /// Construct an edit. Effect zero is off and has no parameter table.
    pub fn new(
        effect: Option<u8>,
        brightness: Option<u8>,
        speed: Option<u8>,
    ) -> Result<Self, InvalidLightingChange> {
        if effect.is_some_and(|value| !Self::supports_effect(value))
            || brightness.is_some_and(|value| !Self::levels().contains(&value))
            || speed.is_some_and(|value| !Self::levels().contains(&value))
            || (effect == Some(0) && (brightness.is_some() || speed.is_some()))
            || (effect.is_none() && brightness.is_none() && speed.is_none())
        {
            return Err(InvalidLightingChange);
        }
        Ok(Self {
            effect,
            brightness,
            speed,
            color_mode: None,
        })
    }

    /// Add an explicit single-color or colorful-mode edit.
    pub fn with_color_mode(
        mut self,
        color_mode: AulaColorModeChange,
    ) -> Result<Self, InvalidLightingChange> {
        if self.effect == Some(0) {
            return Err(InvalidLightingChange);
        }
        self.color_mode = Some(color_mode);
        Ok(self)
    }

    /// OEM built-in effects, including zero for off.
    #[must_use]
    pub fn effects() -> std::ops::RangeInclusive<u8> {
        0..=18
    }

    /// Brightness and speed levels offered by built-in effects.
    #[must_use]
    pub fn levels() -> std::ops::RangeInclusive<u8> {
        0..=4
    }

    /// Whether an identifier has supported built-in semantics.
    #[must_use]
    pub fn supports_effect(effect: u8) -> bool {
        Self::effects().contains(&effect)
    }

    /// Requested effect, if changed.
    #[must_use]
    pub fn effect(self) -> Option<u8> {
        self.effect
    }

    /// Requested brightness, if changed.
    #[must_use]
    pub fn brightness(self) -> Option<u8> {
        self.brightness
    }

    /// Requested speed, if changed.
    #[must_use]
    pub fn speed(self) -> Option<u8> {
        self.speed
    }

    /// Requested color behavior, if changed.
    #[must_use]
    pub const fn color_mode(self) -> Option<AulaColorModeChange> {
        self.color_mode
    }
}

/// Number of addressable LEDs in the F87 PRO wireless custom-lighting table.
pub const AULA_PER_KEY_LED_COUNT: usize = 126;

/// One LED color in the F87 PRO wireless custom-lighting table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AulaRgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl AulaRgb {
    /// Construct an RGB color from its three 8-bit channels.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// Return red, green and blue channel values.
    #[must_use]
    pub const fn channels(self) -> [u8; 3] {
        [self.red, self.green, self.blue]
    }
}

/// A complete custom-lighting table; partial updates are not representable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<AulaRgb>", into = "Vec<AulaRgb>")]
pub struct AulaPerKeyRgb(Box<[AulaRgb; AULA_PER_KEY_LED_COUNT]>);

impl AulaPerKeyRgb {
    /// Construct a full 126-LED table in the device's LED-index order.
    pub fn new(colors: Vec<AulaRgb>) -> Result<Self, InvalidAulaPerKeyRgb> {
        colors
            .try_into()
            .map(|colors| Self(Box::new(colors)))
            .map_err(|_| InvalidAulaPerKeyRgb)
    }

    /// Construct a complete table with the same color at every LED index.
    #[must_use]
    pub fn solid(color: AulaRgb) -> Self {
        Self(Box::new([color; AULA_PER_KEY_LED_COUNT]))
    }

    /// All LED colors in the device's index order.
    #[must_use]
    pub const fn colors(&self) -> &[AulaRgb; AULA_PER_KEY_LED_COUNT] {
        &self.0
    }

    /// Color at one LED index, if the index is in range.
    #[must_use]
    pub fn color(&self, index: usize) -> Option<AulaRgb> {
        self.0.get(index).copied()
    }

    /// Return a full table with one LED changed, preserving every other color.
    #[must_use]
    pub fn with_color(mut self, index: usize, color: AulaRgb) -> Option<Self> {
        let slot = self.0.get_mut(index)?;
        *slot = color;
        Some(self)
    }
}

impl TryFrom<Vec<AulaRgb>> for AulaPerKeyRgb {
    type Error = InvalidAulaPerKeyRgb;

    fn try_from(colors: Vec<AulaRgb>) -> Result<Self, Self::Error> {
        Self::new(colors)
    }
}

impl From<AulaPerKeyRgb> for Vec<AulaRgb> {
    fn from(table: AulaPerKeyRgb) -> Self {
        Vec::from(*table.0)
    }
}

/// Versioned, device-independent JSON profile for a complete AULA RGB table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "UncheckedAulaPerKeyRgbProfile",
    into = "UncheckedAulaPerKeyRgbProfile"
)]
pub struct AulaPerKeyRgbProfile {
    colors: AulaPerKeyRgb,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedAulaPerKeyRgbProfile {
    format: String,
    version: u8,
    colors: AulaPerKeyRgb,
}

impl AulaPerKeyRgbProfile {
    /// Construct a profile from a complete table.
    #[must_use]
    pub const fn new(colors: AulaPerKeyRgb) -> Self {
        Self { colors }
    }

    /// The complete 126-entry color table in wireless LED-index order.
    #[must_use]
    pub const fn colors(&self) -> &AulaPerKeyRgb {
        &self.colors
    }
}

impl TryFrom<UncheckedAulaPerKeyRgbProfile> for AulaPerKeyRgbProfile {
    type Error = InvalidAulaPerKeyRgbProfile;

    fn try_from(value: UncheckedAulaPerKeyRgbProfile) -> Result<Self, Self::Error> {
        if value.format != "openhid-aula-per-key-rgb" || value.version != 1 {
            return Err(InvalidAulaPerKeyRgbProfile);
        }
        Ok(Self::new(value.colors))
    }
}

impl From<AulaPerKeyRgbProfile> for UncheckedAulaPerKeyRgbProfile {
    fn from(profile: AulaPerKeyRgbProfile) -> Self {
        Self {
            format: "openhid-aula-per-key-rgb".into(),
            version: 1,
            colors: profile.colors,
        }
    }
}

/// A per-key RGB profile has an unsupported format identifier or version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unsupported AULA per-key RGB profile format or version")]
pub struct InvalidAulaPerKeyRgbProfile;

/// A custom-lighting table with anything other than 126 LED entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("an AULA per-key RGB table must contain exactly 126 LED colors")]
pub struct InvalidAulaPerKeyRgb;

/// Explicit persistence policy for one recoverable lighting transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingMode {
    /// Stage and verify the edit without issuing SAVE.
    Apply,
    /// Verify, SAVE, and read back; includes an already-staged snapshot.
    Save,
    /// Stage/verify and restore/verify the full snapshot without SAVE.
    Preview,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_timeout_and_settings_validate_at_serialization_boundaries() {
        let expected = [
            (60, 2),
            (90, 3),
            (120, 4),
            (180, 6),
            (240, 8),
            (300, 10),
            (600, 20),
            (900, 30),
            (1200, 40),
        ];
        for (seconds, wire) in expected {
            let timeout = SleepTimeout::from_seconds(seconds).unwrap();
            assert_eq!(timeout.wire_value(), wire);
            assert_eq!(SleepTimeout::from_wire(wire), Some(timeout));
            let change = AulaChange::new(None, Some(timeout)).unwrap();
            let encoded = toml::to_string(&change).unwrap();
            assert_eq!(toml::from_str::<AulaChange>(&encoded).unwrap(), change);
        }
        for seconds in [0, 1, 61, 301, 1201, u16::MAX] {
            assert!(SleepTimeout::from_seconds(seconds).is_none());
            toml::from_str::<AulaChange>(&format!("sleep_timeout = {seconds}")).unwrap_err();
        }
        AulaChange::new(None, None).unwrap_err();
        toml::from_str::<AulaChange>("").unwrap_err();
    }

    #[test]
    fn invalid_edits_cannot_be_constructed_or_deserialized() {
        for (effect, brightness, speed, encoded) in [
            (None, None, None, ""),
            (Some(19), None, None, "effect = 19"),
            (None, Some(5), None, "brightness = 5"),
            (None, None, Some(255), "speed = 255"),
            (Some(0), Some(1), None, "effect = 0\nbrightness = 1"),
            (Some(0), None, Some(1), "effect = 0\nspeed = 1"),
        ] {
            LightingChange::new(effect, brightness, speed).unwrap_err();
            assert!(
                toml::from_str::<LightingChange>(encoded).is_err(),
                "{encoded}"
            );
        }
    }

    #[test]
    fn validated_edits_keep_optional_fields_and_roundtrip() {
        for change in [
            LightingChange::new(Some(0), None, None).unwrap(),
            LightingChange::new(Some(18), Some(4), Some(0)).unwrap(),
            LightingChange::new(None, None, Some(4)).unwrap(),
        ] {
            let encoded = toml::to_string(&change).unwrap();
            assert_eq!(toml::from_str::<LightingChange>(&encoded).unwrap(), change);
        }
    }

    #[test]
    fn per_key_rgb_requires_and_roundtrips_a_complete_table() {
        #[derive(Deserialize, Serialize)]
        struct Document {
            colors: AulaPerKeyRgb,
        }

        let colors: Vec<_> = (0..AULA_PER_KEY_LED_COUNT)
            .map(|index| {
                let led = u8::try_from(index).unwrap();
                AulaRgb::new(led, led.wrapping_add(1), led.wrapping_add(2))
            })
            .collect();
        let table = AulaPerKeyRgb::new(colors.clone()).unwrap();
        let document = Document {
            colors: table.clone(),
        };
        let encoded = toml::to_string(&document).unwrap();
        assert_eq!(toml::from_str::<Document>(&encoded).unwrap().colors, table);
        assert_eq!(table.colors().len(), AULA_PER_KEY_LED_COUNT);
        let changed = table
            .clone()
            .with_color(17, AulaRgb::new(255, 0, 0))
            .unwrap();
        assert_eq!(changed.color(17), Some(AulaRgb::new(255, 0, 0)));
        assert_eq!(changed.color(18), table.color(18));
        assert!(
            changed
                .with_color(AULA_PER_KEY_LED_COUNT, AulaRgb::new(0, 0, 0))
                .is_none()
        );
        AulaPerKeyRgb::new(colors[..AULA_PER_KEY_LED_COUNT - 1].to_vec()).unwrap_err();
        let mut oversized = colors;
        oversized.push(AulaRgb::new(0, 0, 0));
        AulaPerKeyRgb::new(oversized).unwrap_err();
    }
}
