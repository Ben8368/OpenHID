//! Fn action conversion and migration for the F87 PRO OEM local profile.

use super::{AULA_F87_KEY_POSITIONS, AULA_FN_KEY_COUNT, AULA_OEM_FN_RECORD_COUNT};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const OEM_PROFILE_LENGTH: usize = 0x3400;
const OEM_PROFILE_RECORD_MAGIC: u32 = 0xfcaa_1245;
const BASE_RECORDS_OFFSET: usize = 0x3c;
const FN_RECORDS_OFFSET: usize = 0x940;
const FN_RECORD_LENGTH: usize = 0x10;
const FN_ACTION_OFFSET: usize = 4;
const BASE_ACTION_OFFSET: usize = 4;
const PHYSICAL_KEY_COUNT: usize = AULA_F87_KEY_POSITIONS.len();
const OPAQUE_RECORD_LENGTH: usize = FN_RECORD_LENGTH - 4;
/// OEM profile action whose separate parameter is a modifier, VK, and 16-bit VK.
pub const AULA_OEM_KEY_COMBINATION_ACTION: [u8; 4] = [0xa4, 0x00, 0x00, 0x08];
const OEM_FN_DEFAULTS: &[(u8, u32)] = &[
    (0, 0x0700_0004),
    (12, 0x0200_0070),
    (18, 0x0200_006f),
    (24, 0x0004_002b),
    (30, 0x0004_0029),
    (36, 0x0803_0200),
    (42, 0x0803_0100),
    (48, 0x0200_00b6),
    (54, 0x0200_00cd),
    (60, 0x0200_00b5),
    (66, 0x0200_00e2),
    (72, 0x0200_00ea),
    (78, 0x0200_00e9),
    (1, 0x0700_0008),
    (7, 0x0700_0005),
    (13, 0x0700_0006),
    (19, 0x0700_0007),
    (85, 0x0800_0000),
    (86, 0x0802_0000),
    (8, 0x0700_0019),
    (14, 0x0700_0018),
    (20, 0x0700_001a),
    (26, 0x0700_001b),
    (33, 0x0700_000a),
    (4, 0x0800_0002),
    (5, 0x0804_0002),
    (10, 0x0802_0002),
    (11, 0x0700_0001),
    (53, 0x0803_0001),
    (64, 0x0802_0001),
    (82, 0x0800_0001),
    (83, 0x0804_0001),
    (89, 0x0804_0200),
    (95, 0x0803_0200),
    (101, 0x0804_0100),
    (94, 0x0803_0100),
    (34, 0x0700_000c),
];

/// Pair an OEM profile modifier record with its wireless/editor action.
///
/// The installed F87 `KB.ini` stores these seven physical modifiers as a
/// modifier bit plus a Windows virtual-key value. The keymap editor represents
/// a modifier as the modifier bit in byte 1. Keep the conversion table in one
/// place so OEM import and export stay inverse operations.
const fn modifier_action_pair(action: [u8; 4]) -> Option<([u8; 4], [u8; 4])> {
    match action {
        [0x01, 0, 0, 0x11] | [0, 0x01, 0, 0] => Some(([0x01, 0, 0, 0x11], [0, 0x01, 0, 0])),
        [0x02, 0, 0, 0x10] | [0, 0x02, 0, 0] => Some(([0x02, 0, 0, 0x10], [0, 0x02, 0, 0])),
        [0x04, 0, 0, 0x12] | [0, 0x04, 0, 0] => Some(([0x04, 0, 0, 0x12], [0, 0x04, 0, 0])),
        [0x08, 0, 0, 0x5b] | [0, 0x08, 0, 0] => Some(([0x08, 0, 0, 0x5b], [0, 0x08, 0, 0])),
        [0x10, 0, 0, 0xa3] | [0, 0x10, 0, 0] => Some(([0x10, 0, 0, 0xa3], [0, 0x10, 0, 0])),
        [0x20, 0, 0, 0xa1] | [0, 0x20, 0, 0] => Some(([0x20, 0, 0, 0xa1], [0, 0x20, 0, 0])),
        [0x40, 0, 0, 0xa5] | [0, 0x40, 0, 0] => Some(([0x40, 0, 0, 0xa5], [0, 0x40, 0, 0])),
        _ => None,
    }
}

fn validate_profile_dct_container(bytes: &[u8]) -> Result<(), InvalidAulaOemProfile> {
    if bytes.len() != OEM_PROFILE_LENGTH {
        return Err(InvalidAulaOemProfile::Length);
    }
    let record_magic = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if record_magic != OEM_PROFILE_RECORD_MAGIC || bytes[..4] != bytes[8..12] {
        return Err(InvalidAulaOemProfile::Header);
    }
    Ok(())
}

fn validate_base_record_layout(bytes: &[u8]) -> Result<(), InvalidAulaOemProfile> {
    for (key_index, position) in AULA_F87_KEY_POSITIONS.iter().enumerate() {
        let record_offset = BASE_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
        let matrix_slot = position.matrix_slot();
        if bytes[record_offset + 3] != 0x02
            || bytes[record_offset + 12] != matrix_slot
            || bytes[record_offset + 13] != matrix_slot
        {
            return Err(InvalidAulaOemProfile::BaseRecordMapping);
        }
    }
    Ok(())
}

/// Build the F87 PRO factory Fn layer shipped in the installed OEM `KB.ini`.
///
/// The default table uses the OEM Fn1 profile's packed four-byte actions and
/// converts them through the same boundary as imported `.dct` profiles. Empty
/// Fn entries remain unbound. This is a local profile aid; it performs no
/// device I/O and does not provide a factory base-layer snapshot.
#[must_use]
pub fn aula_f87_oem_default_fn_layer() -> Vec<[u8; 4]> {
    let mut entries = vec![[0; 4]; AULA_FN_KEY_COUNT];
    for &(slot, packed_action) in OEM_FN_DEFAULTS {
        let index = usize::from(slot);
        if let Some(entry) = entries.get_mut(index) {
            *entry = AulaOemFnLayer::matrix_action_from_profile_action(packed_action.to_le_bytes());
        }
    }
    entries
}

/// Build OEM-format factory Fn actions in physical-key record order.
///
/// This retains the OEM action page where its matrix representation is
/// ambiguous with a standard keyboard usage.
#[must_use]
pub fn aula_f87_oem_default_fn_actions() -> AulaOemFnActions {
    let mut actions = vec![Some([0; 4]); PHYSICAL_KEY_COUNT];
    for &(slot, packed_action) in OEM_FN_DEFAULTS {
        if let Some(record) = AULA_F87_KEY_POSITIONS
            .iter()
            .position(|position| position.matrix_slot() == slot)
            && let Some(action) = actions.get_mut(record)
        {
            *action = Some(packed_action.to_le_bytes());
        }
    }
    AulaOemFnActions(actions)
}

/// Fn-layer assignments decoded from the installed F87 PRO `profile.dct` format.
///
/// This is a local-profile migration format, not a device backup. It contains
/// only OEM Fn assignments; callers must provide a separately captured base
/// matrix before constructing a complete [`super::AulaKeymap`]. Recognized USB
/// keyboard and consumer actions are converted to the wireless matrix layout;
/// unknown records remain raw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaOemFnLayer {
    entries: [[u8; 4]; AULA_FN_KEY_COUNT],
    records: AulaOemFnRecords,
    actions: AulaOemFnActions,
}

/// Base-layer assignments decoded from the installed F87 PRO `profile.dct`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaOemBaseLayer {
    entries: [[u8; 4]; super::AULA_BASE_KEY_COUNT],
}

/// Opaque per-key fields from the installed OEM profile, excluding each action.
///
/// These bytes are retained across profile editing because their meanings have
/// not all been identified. They are ordered by the OEM file's physical-key
/// records, not by wireless matrix slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "Vec<[u8; OPAQUE_RECORD_LENGTH]>",
    into = "Vec<[u8; OPAQUE_RECORD_LENGTH]>"
)]
pub struct AulaOemFnRecords(Vec<[u8; OPAQUE_RECORD_LENGTH]>);

/// Original OEM Fn actions retained to disambiguate opaque actions on export.
///
/// Entries are in physical-key record order. `None` marks an action explicitly
/// edited in OpenHID and should be encoded from its current matrix value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Option<[u8; 4]>>", into = "Vec<Option<[u8; 4]>>")]
pub struct AulaOemFnActions(Vec<Option<[u8; 4]>>);

/// The installed OEM editor's three fields for a `0x08/0xA4` combo action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AulaOemKeyCombination {
    modify: u8,
    vk1: u8,
    vk2: u16,
}

impl AulaOemKeyCombination {
    /// Construct the OEM editor's `modify`, `vk1`, and `vk2` parameter fields.
    #[must_use]
    pub const fn new(modify: u8, vk1: u8, vk2: u16) -> Self {
        Self { modify, vk1, vk2 }
    }

    /// Decode the four-byte OEM parameter stored after a Fn record's action.
    #[must_use]
    pub const fn from_parameter(bytes: [u8; 4]) -> Self {
        Self {
            modify: bytes[0],
            vk1: bytes[1],
            vk2: u16::from_le_bytes([bytes[2], bytes[3]]),
        }
    }

    /// Encode the OEM parameter in its on-disk little-endian layout.
    #[must_use]
    pub const fn to_parameter(self) -> [u8; 4] {
        let vk2 = self.vk2.to_le_bytes();
        [self.modify, self.vk1, vk2[0], vk2[1]]
    }

    /// The OEM `modify` selector value.
    #[must_use]
    pub const fn modify(self) -> u8 {
        self.modify
    }

    /// The first OEM virtual-key selector value.
    #[must_use]
    pub const fn vk1(self) -> u8 {
        self.vk1
    }

    /// The second OEM virtual-key selector value.
    #[must_use]
    pub const fn vk2(self) -> u16 {
        self.vk2
    }
}

impl AulaOemFnActions {
    fn from_profile_dct(bytes: &[u8]) -> Self {
        let actions = AULA_F87_KEY_POSITIONS
            .iter()
            .enumerate()
            .map(|(key_index, _)| {
                let record_offset = FN_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
                let action_start = record_offset + FN_ACTION_OFFSET;
                let mut action = [0; 4];
                action.copy_from_slice(&bytes[action_start..action_start + 4]);
                Some(action)
            })
            .collect();
        Self(actions)
    }

    /// Original OEM action for a physical-key record, unless it was edited.
    #[must_use]
    pub fn action(&self, record_index: usize) -> Option<[u8; 4]> {
        self.0.get(record_index).copied().flatten()
    }

    /// Mark the Fn action for a matrix slot as explicitly edited.
    pub fn clear_matrix_slot(&mut self, slot: usize) -> Option<()> {
        let record_index = AULA_F87_KEY_POSITIONS
            .iter()
            .position(|position| usize::from(position.matrix_slot()) == slot)?;
        *self.0.get_mut(record_index)? = None;
        Some(())
    }
}

impl TryFrom<Vec<Option<[u8; 4]>>> for AulaOemFnActions {
    type Error = InvalidAulaOemProfile;

    fn try_from(actions: Vec<Option<[u8; 4]>>) -> Result<Self, Self::Error> {
        if actions.len() != PHYSICAL_KEY_COUNT {
            return Err(InvalidAulaOemProfile::FnActionCount);
        }
        Ok(Self(actions))
    }
}

impl From<AulaOemFnActions> for Vec<Option<[u8; 4]>> {
    fn from(actions: AulaOemFnActions) -> Self {
        actions.0
    }
}

impl AulaOemFnRecords {
    fn from_profile_dct(bytes: &[u8]) -> Self {
        let records = AULA_F87_KEY_POSITIONS
            .iter()
            .enumerate()
            .map(|(key_index, _)| {
                let record_offset = FN_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
                let mut opaque = [0; OPAQUE_RECORD_LENGTH];
                opaque[..FN_ACTION_OFFSET]
                    .copy_from_slice(&bytes[record_offset..record_offset + FN_ACTION_OFFSET]);
                opaque[FN_ACTION_OFFSET..].copy_from_slice(
                    &bytes[record_offset + FN_ACTION_OFFSET + 4..record_offset + FN_RECORD_LENGTH],
                );
                opaque
            })
            .collect();
        Self(records)
    }

    /// Return the retained OEM fields in physical-key record order.
    #[must_use]
    pub fn records(&self) -> &[[u8; OPAQUE_RECORD_LENGTH]] {
        &self.0
    }

    /// Copy one opaque OEM Fn row by its physical-key record index.
    #[must_use]
    pub fn record(&self, index: usize) -> Option<[u8; OPAQUE_RECORD_LENGTH]> {
        self.0.get(index).copied()
    }

    /// Decode the parameter bytes for an OEM key-combination action.
    ///
    /// The caller must first confirm that the corresponding action is
    /// [`AULA_OEM_KEY_COMBINATION_ACTION`].
    #[must_use]
    pub fn key_combination(&self, index: usize) -> Option<AulaOemKeyCombination> {
        let record = self.0.get(index)?;
        Some(AulaOemKeyCombination::from_parameter([
            record[8], record[9], record[10], record[11],
        ]))
    }

    /// Replace the parameter bytes for an OEM key-combination action.
    ///
    /// The caller must first confirm that the corresponding action is
    /// [`AULA_OEM_KEY_COMBINATION_ACTION`].
    pub fn replace_key_combination(
        &mut self,
        index: usize,
        combination: AulaOemKeyCombination,
    ) -> Option<()> {
        let record = self.0.get_mut(index)?;
        record[8..12].copy_from_slice(&combination.to_parameter());
        Some(())
    }

    /// Replace one opaque OEM Fn row by its physical-key record index.
    ///
    /// The action itself is stored separately in the keymap. `None` means the
    /// index is outside the 87 physical-key rows retained from `profile.dct`.
    pub fn replace_record(
        &mut self,
        index: usize,
        record: [u8; OPAQUE_RECORD_LENGTH],
    ) -> Option<()> {
        *self.0.get_mut(index)? = record;
        Some(())
    }
}

impl TryFrom<Vec<[u8; OPAQUE_RECORD_LENGTH]>> for AulaOemFnRecords {
    type Error = InvalidAulaOemProfile;

    fn try_from(records: Vec<[u8; OPAQUE_RECORD_LENGTH]>) -> Result<Self, Self::Error> {
        if records.len() != PHYSICAL_KEY_COUNT {
            return Err(InvalidAulaOemProfile::FnRecordCount);
        }
        Ok(Self(records))
    }
}

impl From<AulaOemFnRecords> for Vec<[u8; OPAQUE_RECORD_LENGTH]> {
    fn from(records: AulaOemFnRecords) -> Self {
        records.0
    }
}

impl AulaOemFnLayer {
    /// Convert one OEM profile action to the wireless/editor matrix layout.
    ///
    /// The OEM profile stores USB keyboard and consumer actions as
    /// `[usage_low, usage_high, 0, page]`; the matrix editor stores the
    /// consumer usage's high and low bytes in positions 2 and 3. Unknown
    /// actions and the all-zero disabled value are preserved byte-for-byte.
    #[must_use]
    pub const fn matrix_action_from_profile_action(action: [u8; 4]) -> [u8; 4] {
        match action {
            [usage, 0, 0, 0x07] if usage != 0 => [0, 0, 0, usage],
            [usage, usage_high, 0, 0x02] if usage != 0 => [0x02, 0, usage_high, usage],
            _ => action,
        }
    }

    /// Convert one wireless/editor matrix action to the OEM profile layout.
    ///
    /// Actions outside the recognized USB keyboard and consumer pages remain unchanged.
    #[must_use]
    pub const fn profile_action_from_matrix_action(action: [u8; 4]) -> [u8; 4] {
        match action {
            [0, 0, 0, usage] if usage != 0 => [usage, 0, 0, 0x07],
            [0x02, 0, usage_high, usage] if usage != 0 => [usage, usage_high, 0, 0x02],
            _ => action,
        }
    }

    /// Decode Fn assignments from a `0x3400`-byte F87 PRO `profile.dct` file.
    ///
    /// The 87 OEM key records are mapped to wireless matrix slots using the
    /// installed keyboard layout. Unknown records in the remaining OEM key
    /// range are rejected instead of being silently discarded.
    pub fn from_profile_dct(bytes: &[u8]) -> Result<Self, InvalidAulaOemProfile> {
        validate_profile_dct_container(bytes)?;
        validate_base_record_layout(bytes)?;

        let mut entries = [[0; 4]; AULA_FN_KEY_COUNT];
        for (key_index, position) in AULA_F87_KEY_POSITIONS.iter().enumerate() {
            let record_offset = FN_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
            let action_start = record_offset + FN_ACTION_OFFSET;
            let action_end = action_start + 4;
            let slot = usize::from(position.matrix_slot());
            let entry = entries
                .get_mut(slot)
                .ok_or(InvalidAulaOemProfile::KeySlot)?;
            let mut action = [0; 4];
            action.copy_from_slice(&bytes[action_start..action_end]);
            *entry = Self::matrix_action_from_profile_action(action);
        }

        let unsupported_start = FN_RECORDS_OFFSET + PHYSICAL_KEY_COUNT * FN_RECORD_LENGTH;
        let unsupported_end = FN_RECORDS_OFFSET + AULA_OEM_FN_RECORD_COUNT * FN_RECORD_LENGTH;
        if bytes[unsupported_start..unsupported_end]
            .as_chunks::<FN_RECORD_LENGTH>()
            .0
            .iter()
            .any(|record| record.iter().any(|byte| *byte != 0))
        {
            return Err(InvalidAulaOemProfile::UnsupportedKeyRecords);
        }

        Ok(Self {
            entries,
            records: AulaOemFnRecords::from_profile_dct(bytes),
            actions: AulaOemFnActions::from_profile_dct(bytes),
        })
    }

    /// Replace mapped Fn actions and optionally restore their retained opaque row fields.
    ///
    /// The OEM file contains records for the 87 physical keys. Fn matrix entries
    /// without a corresponding physical key are not serialized into this file.
    /// Bytes outside those records remain from the supplied template.
    pub fn write_to_profile_dct(
        template: &[u8],
        fn_layer: &[[u8; 4]],
        records: Option<&AulaOemFnRecords>,
        actions: Option<&AulaOemFnActions>,
    ) -> Result<Vec<u8>, InvalidAulaOemProfile> {
        if fn_layer.len() != AULA_FN_KEY_COUNT {
            return Err(InvalidAulaOemProfile::FnLayerLength);
        }
        validate_profile_dct_container(template)?;

        let mut updated = template.to_vec();
        if let Some(records) = records {
            if records.0.len() != PHYSICAL_KEY_COUNT {
                return Err(InvalidAulaOemProfile::FnRecordCount);
            }
            for (key_index, opaque) in records.0.iter().enumerate() {
                let record_offset = FN_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
                updated[record_offset..record_offset + FN_ACTION_OFFSET]
                    .copy_from_slice(&opaque[..FN_ACTION_OFFSET]);
                updated[record_offset + FN_ACTION_OFFSET + 4..record_offset + FN_RECORD_LENGTH]
                    .copy_from_slice(&opaque[FN_ACTION_OFFSET..]);
            }
        }
        for (key_index, position) in AULA_F87_KEY_POSITIONS.iter().enumerate() {
            let record_offset = FN_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
            let action_start = record_offset + FN_ACTION_OFFSET;
            let action_end = action_start + 4;
            let slot = usize::from(position.matrix_slot());
            let source_action = match actions {
                Some(actions) => actions.action(key_index),
                None => Some(read_fn_profile_action(template, key_index)?),
            };
            let action = source_action
                .filter(|action| Self::matrix_action_from_profile_action(*action) == fn_layer[slot])
                .unwrap_or_else(|| Self::profile_action_from_matrix_action(fn_layer[slot]));
            updated[action_start..action_end].copy_from_slice(&action);
        }
        Ok(updated)
    }

    /// Four-byte Fn assignments in wireless matrix-slot order.
    #[must_use]
    pub const fn entries(&self) -> &[[u8; 4]; AULA_FN_KEY_COUNT] {
        &self.entries
    }

    /// Opaque OEM fields retained from the source profile.
    #[must_use]
    pub const fn records(&self) -> &AulaOemFnRecords {
        &self.records
    }

    /// Original OEM action records, including opaque actions preserved as-is.
    #[must_use]
    pub const fn actions(&self) -> &AulaOemFnActions {
        &self.actions
    }

    /// Combine the OEM Fn assignments with a separately read base matrix.
    #[must_use]
    pub fn merge_with_base_matrix(&self, base: &super::AulaBaseMatrix) -> super::AulaKeymap {
        super::AulaKeymap {
            base: base.entries.clone(),
            fn_layer: self.entries.to_vec(),
        }
    }
}

fn read_fn_profile_action(
    profile: &[u8],
    key_index: usize,
) -> Result<[u8; 4], InvalidAulaOemProfile> {
    let record_offset = FN_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
    let action_start = record_offset + FN_ACTION_OFFSET;
    let action_bytes = profile
        .get(action_start..action_start + 4)
        .ok_or(InvalidAulaOemProfile::Length)?;
    let mut action = [0; 4];
    action.copy_from_slice(action_bytes);
    Ok(action)
}

impl AulaOemBaseLayer {
    /// Decode the 87 physical-key base assignments from an F87 PRO profile.
    pub fn from_profile_dct(bytes: &[u8]) -> Result<Self, InvalidAulaOemProfile> {
        validate_profile_dct_container(bytes)?;
        validate_base_record_layout(bytes)?;
        let mut entries = [[0; 4]; super::AULA_BASE_KEY_COUNT];
        for (key_index, position) in AULA_F87_KEY_POSITIONS.iter().enumerate() {
            let record_offset = BASE_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
            let action_start = record_offset + BASE_ACTION_OFFSET;
            let slot = usize::from(position.matrix_slot());
            let entry = entries
                .get_mut(slot)
                .ok_or(InvalidAulaOemProfile::KeySlot)?;
            let mut action = [0; 4];
            action.copy_from_slice(&bytes[action_start..action_start + 4]);
            *entry = Self::matrix_action_from_profile_action(action);
        }
        Ok(Self { entries })
    }

    /// Convert an OEM base key assignment to the wireless/editor matrix layout.
    ///
    /// Plain USB keyboard usages move from byte 1 to byte 3. The seven
    /// installed OEM modifier encodings map to the editor's modifier-bit form;
    /// unknown actions and combinations remain unchanged.
    #[must_use]
    pub const fn matrix_action_from_profile_action(action: [u8; 4]) -> [u8; 4] {
        match action {
            [0, usage, 0, 0] if usage != 0 => [0, 0, 0, usage],
            _ => match modifier_action_pair(action) {
                Some((_, matrix_action)) => matrix_action,
                None => action,
            },
        }
    }

    /// Convert a wireless/editor base assignment to the OEM profile layout.
    ///
    /// Recognized modifier-bit actions return to the OEM modifier-mask and
    /// Windows virtual-key encoding; unknown actions remain unchanged.
    #[must_use]
    pub const fn profile_action_from_matrix_action(action: [u8; 4]) -> [u8; 4] {
        match action {
            [0, 0, 0, usage] if usage != 0 => [0, usage, 0, 0],
            _ => match modifier_action_pair(action) {
                Some((profile_action, _)) => profile_action,
                None => action,
            },
        }
    }

    /// The 126 base assignments in wireless matrix-slot order.
    #[must_use]
    pub const fn entries(&self) -> &[[u8; 4]; super::AULA_BASE_KEY_COUNT] {
        &self.entries
    }
}

/// Apply base and Fn assignments to a validated OEM profile template.
///
/// When the template's Fn action still matches the supplied matrix value, its
/// original OEM bytes are retained, including for profiles without provenance.
pub fn write_keymap_to_profile_dct(
    template: &[u8],
    keymap: &super::AulaKeymap,
    fn_records: Option<&AulaOemFnRecords>,
) -> Result<Vec<u8>, InvalidAulaOemProfile> {
    write_keymap_to_profile_dct_with_fn_actions(template, keymap, fn_records, None)
}

/// Apply base and Fn assignments while retaining the source OEM Fn actions when unchanged.
///
/// If provenance metadata is absent, the template's own action is used as the
/// source. A present metadata entry of `None` means that action was explicitly
/// edited and should be encoded from the current matrix value.
pub fn write_keymap_to_profile_dct_with_fn_actions(
    template: &[u8],
    keymap: &super::AulaKeymap,
    fn_records: Option<&AulaOemFnRecords>,
    fn_actions: Option<&AulaOemFnActions>,
) -> Result<Vec<u8>, InvalidAulaOemProfile> {
    validate_profile_dct_container(template)?;
    validate_base_record_layout(template)?;
    let mut updated =
        AulaOemFnLayer::write_to_profile_dct(template, keymap.fn_layer(), fn_records, fn_actions)?;
    for (key_index, position) in AULA_F87_KEY_POSITIONS.iter().enumerate() {
        let record_offset = BASE_RECORDS_OFFSET + key_index * FN_RECORD_LENGTH;
        let action_start = record_offset + BASE_ACTION_OFFSET;
        let slot = usize::from(position.matrix_slot());
        let action = AulaOemBaseLayer::profile_action_from_matrix_action(keymap.base()[slot]);
        updated[action_start..action_start + 4].copy_from_slice(&action);
    }
    Ok(updated)
}

/// The OEM local profile is malformed or contains key records this importer cannot map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemProfile {
    /// The profile is not the verified fixed-size F87 PRO file format.
    #[error("the AULA OEM profile must be exactly 0x3400 bytes")]
    Length,
    /// The profile header magic or duplicated outer field is invalid.
    #[error("the AULA OEM profile header is invalid")]
    Header,
    /// A base-key record's type or duplicated matrix slot does not match the F87 layout.
    #[error("the AULA OEM base-key records do not match the F87 matrix layout")]
    BaseRecordMapping,
    /// A physical key maps outside the wireless Fn matrix.
    #[error("the AULA OEM profile contains a key outside the Fn matrix")]
    KeySlot,
    /// The profile contains non-empty OEM records beyond the 87-key layout.
    #[error("the AULA OEM profile contains unmapped key records")]
    UnsupportedKeyRecords,
    /// The supplied wireless Fn matrix has the wrong length.
    #[error("the AULA Fn matrix must contain exactly 126 entries")]
    FnLayerLength,
    /// The OEM companion-field table does not contain exactly 87 physical-key records.
    #[error("the AULA OEM Fn metadata must contain exactly 87 key records")]
    FnRecordCount,
    /// The OEM Fn action table does not contain exactly 87 physical-key records.
    #[error("the AULA OEM Fn action table must contain exactly 87 key records")]
    FnActionCount,
}
