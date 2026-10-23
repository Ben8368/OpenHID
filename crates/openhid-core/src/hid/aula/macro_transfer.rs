//! Static reconstruction of an OEM macro per-node encoder.

use thiserror::Error;

use super::AulaOemMacroFile;

const MACRO_NAME_OFFSET: usize = 0x14;
const MACRO_NAME_MAX_UNITS: usize = 0x1e;
const MACRO_NAME_BYTES: usize = MACRO_NAME_MAX_UNITS * 2;
const MACRO_EVENT_VALUE_OFFSET: usize = 4;
const MACRO_EVENT_AUXILIARY_OFFSET: usize = 2;
const MACRO_EVENT_SPECIAL_OFFSET: usize = 8;
const MACRO_EVENT_ENCODED_LENGTH: usize = 4;
const MAX_EVENT_VALUE: i32 = 0x000f_ffff;
const MACRO_TRANSFER_TABLE_LIMIT: usize = 0x2800;

/// Translate one local OEM macro node into the per-node buffer built by the
/// installed OEM driver's `StMacro_To_HdMacro` path.
///
/// This is a host-free reconstruction of the OEM's local serializer. It does
/// not create the aggregate device table or a HID report, and does not prove
/// that a device accepts the result. Event codes whose behavior depends on an
/// OEM context object are rejected rather than guessed.
pub fn aula_oem_macro_transfer_block(
    macro_file: &AulaOemMacroFile,
) -> Result<Vec<u8>, InvalidAulaOemMacroTransferBlock> {
    let name = &macro_file.header()[MACRO_NAME_OFFSET..MACRO_NAME_OFFSET + MACRO_NAME_BYTES];
    let name_length = name
        .as_chunks::<2>()
        .0
        .iter()
        .take(MACRO_NAME_MAX_UNITS)
        .take_while(|unit| **unit != [0, 0])
        .count()
        * 2;
    let name = &name[..name_length];

    let mut block = Vec::with_capacity(
        name_length + 1 + macro_file.records().len() * MACRO_EVENT_ENCODED_LENGTH,
    );
    block.extend_from_slice(name);
    block.push(
        u8::try_from(name_length).map_err(|_| InvalidAulaOemMacroTransferBlock::NameTooLong)?,
    );

    for record in macro_file.records() {
        let event_code = u16::from_le_bytes([record[0], record[1]]);
        if event_code == 0 {
            continue;
        }

        let (action_code, action_class) = encoded_action(*record)?;
        if action_code == 0 {
            return Err(InvalidAulaOemMacroTransferBlock::ZeroActionCode(event_code));
        }
        // The OEM uses a signed `jle` after comparing the 32-bit value with
        // 0xFFFFF, so high-bit-set values pass through unchanged.
        let value = i32::from_le_bytes([
            record[MACRO_EVENT_VALUE_OFFSET],
            record[MACRO_EVENT_VALUE_OFFSET + 1],
            record[MACRO_EVENT_VALUE_OFFSET + 2],
            record[MACRO_EVENT_VALUE_OFFSET + 3],
        ]);
        let value = if value <= MAX_EVENT_VALUE {
            value
        } else {
            MAX_EVENT_VALUE
        };
        let value_bytes = value.to_le_bytes();
        let mut descriptor = (action_class << 4) | (value_bytes[2] & 0x0f);
        if !matches!(action_class, 3..=5)
            && u16::from_le_bytes([
                record[MACRO_EVENT_AUXILIARY_OFFSET],
                record[MACRO_EVENT_AUXILIARY_OFFSET + 1],
            ]) == 0
        {
            descriptor |= 0x80;
        }

        block.extend_from_slice(&[descriptor, value_bytes[1], value_bytes[0], action_code]);
    }

    Ok(block)
}

/// One unique macro's directory entry in an aggregate OEM transfer table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AulaOemMacroTransferEntry {
    node_id: u32,
    index: u32,
    offset: u16,
    length: u16,
}

impl AulaOemMacroTransferEntry {
    /// OEM macro-tree node identifier.
    #[must_use]
    pub const fn node_id(self) -> u32 {
        self.node_id
    }

    /// One-based index assigned in first-reference order.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Byte offset of the encoded macro block in the aggregate table.
    #[must_use]
    pub const fn offset(self) -> u16 {
        self.offset
    }

    /// Encoded macro block length in bytes.
    #[must_use]
    pub const fn length(self) -> u16 {
        self.length
    }
}

/// A reconstructed directory and payload for an ordered set of referenced macros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaOemMacroTransferTable {
    bytes: Vec<u8>,
    entries: Vec<AulaOemMacroTransferEntry>,
}

impl AulaOemMacroTransferTable {
    /// Aggregate directory followed by the per-node macro blocks.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Unique macro entries in the order supplied to the builder.
    #[must_use]
    pub fn entries(&self) -> &[AulaOemMacroTransferEntry] {
        &self.entries
    }
}

/// Build the OEM macro directory and payload in first-reference order.
///
/// The installed driver emits one little-endian `u16` offset and one `u16`
/// length per unique macro, followed by the corresponding per-node blocks.
/// Input order must match the driver's first-reference order across its key
/// banks. Repeated references to the same node ID are deduplicated. The result
/// is limited to the OEM caller's `0x2800`-byte maximum and is not a HID report.
pub fn aula_oem_macro_transfer_table(
    macros_in_reference_order: &[&AulaOemMacroFile],
) -> Result<AulaOemMacroTransferTable, InvalidAulaOemMacroTransferTable> {
    let mut unique: Vec<(&AulaOemMacroFile, Vec<u8>)> =
        Vec::with_capacity(macros_in_reference_order.len());
    for macro_file in macros_in_reference_order {
        if let Some((existing, _)) = unique
            .iter()
            .find(|(existing, _)| existing.node_id() == macro_file.node_id())
        {
            if *existing != *macro_file {
                return Err(InvalidAulaOemMacroTransferTable::ConflictingNodeId(
                    macro_file.node_id(),
                ));
            }
            continue;
        }
        unique.push((*macro_file, aula_oem_macro_transfer_block(macro_file)?));
    }

    let directory_length = unique
        .len()
        .checked_mul(4)
        .ok_or(InvalidAulaOemMacroTransferTable::Length)?;
    let payload_length = unique.iter().try_fold(0_usize, |length, (_, block)| {
        length
            .checked_add(block.len())
            .ok_or(InvalidAulaOemMacroTransferTable::Length)
    })?;
    let total_length = directory_length
        .checked_add(payload_length)
        .ok_or(InvalidAulaOemMacroTransferTable::Length)?;
    if total_length > MACRO_TRANSFER_TABLE_LIMIT {
        return Err(InvalidAulaOemMacroTransferTable::SizeLimit);
    }

    let mut table = Vec::with_capacity(total_length);
    let mut entries = Vec::with_capacity(unique.len());
    let mut payload_offset = directory_length;
    for (ordinal, (macro_file, block)) in unique.iter().enumerate() {
        let offset =
            u16::try_from(payload_offset).map_err(|_| InvalidAulaOemMacroTransferTable::Length)?;
        let length =
            u16::try_from(block.len()).map_err(|_| InvalidAulaOemMacroTransferTable::Length)?;
        let index =
            u32::try_from(ordinal + 1).map_err(|_| InvalidAulaOemMacroTransferTable::Length)?;
        table.extend_from_slice(&offset.to_le_bytes());
        table.extend_from_slice(&length.to_le_bytes());
        entries.push(AulaOemMacroTransferEntry {
            node_id: macro_file.node_id(),
            index,
            offset,
            length,
        });
        payload_offset += block.len();
    }
    for (_, block) in unique {
        table.extend_from_slice(&block);
    }

    Ok(AulaOemMacroTransferTable {
        bytes: table,
        entries,
    })
}

fn encoded_action(record: [u8; 12]) -> Result<(u8, u8), InvalidAulaOemMacroTransferBlock> {
    let event_code = u16::from_le_bytes([record[0], record[1]]);
    let special_value = record[MACRO_EVENT_SPECIAL_OFFSET];
    let action = match event_code {
        0x0010 | 0x00a0 => (0xe1, 1),
        0x0011 | 0x00a2 => (0xe0, 1),
        0x0012 | 0x00a4 => (0xe2, 1),
        0x005b => (0xe3, 1),
        0x005c => (0xe7, 1),
        0x00a1 => (0xe5, 1),
        0x00a3 => (0xe4, 1),
        0x00a5 => (0xe6, 1),
        0x00f0 => (0x01, 2),
        0x00f1 => (0x02, 2),
        0x00f2 => (0x04, 2),
        0x00f3 | 0x00f4 => {
            return Err(InvalidAulaOemMacroTransferBlock::ContextDependentEventCode(
                event_code,
            ));
        }
        0x00f5 => (special_value, 5),
        0x00f6 => (0_u8.wrapping_sub(special_value), 5),
        0x00f8 => (special_value, 3),
        0x00f9 => (special_value, 4),
        _ => (
            key_usage_from_windows_virtual_key(event_code).ok_or(
                InvalidAulaOemMacroTransferBlock::UnsupportedEventCode(event_code),
            )?,
            0,
        ),
    };
    Ok(action)
}

fn key_usage_from_windows_virtual_key(event_code: u16) -> Option<u8> {
    if event_code > u16::from(u8::MAX) {
        return None;
    }
    OEM_KEY_USAGE_BY_VK
        .iter()
        .find_map(|(virtual_key, usage)| (u16::from(*virtual_key) == event_code).then_some(*usage))
}

/// Windows virtual-key codes handled by the recovered OEM macro encoder,
/// paired with their serialized HID usage byte.
///
/// This list supports offline editing choices; it does not prove which device
/// transport accepts the encoded macro block.
#[must_use]
pub const fn aula_oem_macro_mapped_virtual_keys() -> &'static [(u8, u8)] {
    &OEM_KEY_USAGE_BY_VK
}

/// An event cannot be converted by the statically recovered OEM encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemMacroTransferBlock {
    /// The preserved name field exceeds the OEM serializer's 30-unit limit.
    #[error("AULA OEM macro name exceeds the 30-code-unit transfer limit")]
    NameTooLong,
    /// The event code is not present in the driver's mapped-key table.
    #[error("AULA OEM macro event code 0x{0:04X} has no recovered transfer encoding")]
    UnsupportedEventCode(u16),
    /// The OEM skips rows whose encoded action byte is zero.
    #[error("AULA OEM macro event code 0x{0:04X} encodes an empty action")]
    ZeroActionCode(u16),
    /// The driver's encoding depends on a context object not reproduced here.
    #[error("AULA OEM macro event code 0x{0:04X} depends on an unrecovered context")]
    ContextDependentEventCode(u16),
}

/// The ordered macros cannot be represented by the recovered OEM table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemMacroTransferTable {
    /// A macro node ID appeared with conflicting contents.
    #[error("AULA macro node ID 0x{0:08X} appears with conflicting data")]
    ConflictingNodeId(u32),
    /// The table exceeds the installed OEM caller's 0x2800-byte limit.
    #[error("AULA OEM macro transfer table exceeds the 0x2800-byte limit")]
    SizeLimit,
    /// A directory field or aggregate length does not fit the OEM format.
    #[error("AULA OEM macro transfer table length is not representable")]
    Length,
    /// One macro block could not be encoded.
    #[error(transparent)]
    Block(#[from] InvalidAulaOemMacroTransferBlock),
}

// (Windows virtual-key code, HID usage byte) pairs from the installed OEM
// serializer's key-conversion table at VA 0x00607290. Its use by the F87
// wireless `CDevComboFilm` path has not been established. Keep this mapping
// here so callers do not re-derive the OEM transfer decision independently.
const OEM_KEY_USAGE_BY_VK: [(u8, u8); 115] = [
    (0xe2, 0x64),
    (0x1b, 0x29),
    (0x70, 0x3a),
    (0x71, 0x3b),
    (0x72, 0x3c),
    (0x73, 0x3d),
    (0x74, 0x3e),
    (0x75, 0x3f),
    (0x76, 0x40),
    (0x77, 0x41),
    (0x78, 0x42),
    (0x79, 0x43),
    (0x7a, 0x44),
    (0x7b, 0x45),
    (0x7c, 0x68),
    (0x41, 0x04),
    (0x42, 0x05),
    (0x43, 0x06),
    (0x44, 0x07),
    (0x45, 0x08),
    (0x46, 0x09),
    (0x47, 0x0a),
    (0x48, 0x0b),
    (0x49, 0x0c),
    (0x4a, 0x0d),
    (0x4b, 0x0e),
    (0x4c, 0x0f),
    (0x4d, 0x10),
    (0x4e, 0x11),
    (0x4f, 0x12),
    (0x50, 0x13),
    (0x51, 0x14),
    (0x52, 0x15),
    (0x53, 0x16),
    (0x54, 0x17),
    (0x55, 0x18),
    (0x56, 0x19),
    (0x57, 0x1a),
    (0x58, 0x1b),
    (0x59, 0x1c),
    (0x5a, 0x1d),
    (0x31, 0x1e),
    (0x32, 0x1f),
    (0x33, 0x20),
    (0x34, 0x21),
    (0x35, 0x22),
    (0x36, 0x23),
    (0x37, 0x24),
    (0x38, 0x25),
    (0x39, 0x26),
    (0x30, 0x27),
    (0x09, 0x2b),
    (0x14, 0x39),
    (0x0d, 0x28),
    (0x08, 0x2a),
    (0x20, 0x2c),
    (0xdb, 0x2f),
    (0xdd, 0x30),
    (0xdc, 0x31),
    (0xba, 0x33),
    (0xde, 0x34),
    (0xbc, 0x36),
    (0xbe, 0x37),
    (0xbf, 0x38),
    (0xc0, 0x35),
    (0xbb, 0x2e),
    (0xbd, 0x2d),
    (0x2c, 0x46),
    (0x91, 0x47),
    (0x13, 0x48),
    (0x2d, 0x49),
    (0x24, 0x4a),
    (0x21, 0x4b),
    (0x2e, 0x4c),
    (0x23, 0x4d),
    (0x22, 0x4e),
    (0x26, 0x52),
    (0x28, 0x51),
    (0x25, 0x50),
    (0x27, 0x4f),
    (0x61, 0x59),
    (0x62, 0x5a),
    (0x63, 0x5b),
    (0x64, 0x5c),
    (0x65, 0x5d),
    (0x66, 0x5e),
    (0x67, 0x5f),
    (0x68, 0x60),
    (0x69, 0x61),
    (0x60, 0x62),
    (0x6b, 0x57),
    (0x6d, 0x56),
    (0x6a, 0x55),
    (0x6f, 0x54),
    (0x90, 0x53),
    (0xfd, 0x58),
    (0x6e, 0x63),
    (0xa2, 0xe0),
    (0xa0, 0xe1),
    (0xa4, 0xe2),
    (0x5b, 0xe3),
    (0xa3, 0xe4),
    (0xa1, 0xe5),
    (0xa5, 0xe6),
    (0x5c, 0xe7),
    (0x5d, 0x65),
    (0xc2, 0x85),
    (0xc1, 0x87),
    (0xc3, 0x89),
    (0xd8, 0x32),
    (0xda, 0x8a),
    (0xd7, 0x88),
    (0xeb, 0x8b),
    (0xc4, 0x90),
    (0xc5, 0x91),
];
