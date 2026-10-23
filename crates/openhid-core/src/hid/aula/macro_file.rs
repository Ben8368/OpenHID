//! Host-free codec for the installed AULA `.mcr` macro container.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const HEADER_LENGTH: usize = 0x68;
const RECORD_LENGTH: usize = 12;
const RECORD_COUNT_OFFSET: usize = 0x58;
const HEADER_MAGIC: u32 = 0xfacb_99a2;
const NAME_OFFSET: usize = 0x14;
const NAME_LENGTH: usize = 0x40;
const NAME_MAX_UNITS: usize = 29;
const CREATED_FLAG_OFFSET: usize = 0x54;
const TEA_DELTA: u32 = 0x9e37_79b9;
const TEA_KEY: [u32; 4] = [0x0001_0022, 0x0060_f420, 3, 4];
/// Maximum record count traversed by the installed OEM macro serializer.
pub const AULA_OEM_MACRO_RECORD_LIMIT: usize = 0x3f7;
/// Stable format identifier for portable raw AULA macro profiles.
pub const AULA_OEM_MACRO_PROFILE_FORMAT: &str = "openhid-aula-oem-macro";

/// An OEM `.mcr` macro preserving its opaque header fields and 12-byte events.
///
/// Raw event bytes remain intact so importing and exporting can preserve OEM
/// actions. A subset of event codes is classified, but payload meanings and
/// unclassified actions remain unresolved for the F87 PRO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaOemMacroFile {
    header: [u8; HEADER_LENGTH],
    records: Vec<[u8; RECORD_LENGTH]>,
}

/// One OEM macro row in the driver's 12-byte on-disk representation.
///
/// This exposes the recovered storage fields and a partial event-code
/// classification without assigning meanings to unverified payloads or timing
/// values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AulaOemMacroEvent {
    event_code: u16,
    auxiliary: u16,
    value: u16,
    reserved: [u8; 2],
    special_value: u32,
}

/// A partially recovered OEM macro event class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AulaOemMacroEventKind {
    /// The OEM editor skips this row when its code and bytes 4–7 are zero.
    EmptyRow,
    /// Mouse left button (`0xF0`), as labeled by the installed OEM editor.
    MouseLeftButton,
    /// An ordinary mapped input action in the OEM's `0x00`–`0xF4` code range.
    MappedInput,
    /// Insert a delay (`0xF5`); its stored unit and timing semantics are unknown.
    Delay,
    /// The event uses the 32-bit field at offset 8 as its special payload.
    SpecialPayload,
    /// The event code has no recovered selector label or record-decoding class.
    Unknown,
}

const SELECTOR_EVENTS: [(u16, AulaOemMacroEventKind); 5] = [
    (0x00f0, AulaOemMacroEventKind::MouseLeftButton),
    (0x00f5, AulaOemMacroEventKind::Delay),
    (0x00f6, AulaOemMacroEventKind::SpecialPayload),
    (0x00f8, AulaOemMacroEventKind::SpecialPayload),
    (0x00f9, AulaOemMacroEventKind::SpecialPayload),
];

impl std::fmt::Display for AulaOemMacroEventKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::EmptyRow => "empty-row",
            Self::MouseLeftButton => "mouse-left-button",
            Self::MappedInput => "mapped-input",
            Self::Delay => "delay",
            Self::SpecialPayload => "special-payload",
            Self::Unknown => "unknown",
        })
    }
}

impl AulaOemMacroEvent {
    /// Return a raw record observed in the installed OEM editor's sample macro.
    ///
    /// The sample contains one mouse-left selector row with opaque value
    /// `0x0045`. This preserves the observed bytes without assigning meaning
    /// to that value; no other selector has a validated template yet.
    #[must_use]
    pub const fn observed_record_template(event_code: u16) -> Option<[u8; RECORD_LENGTH]> {
        match event_code {
            0x00f0 => Some([
                0xf0, 0x00, 0x00, 0x00, 0x45, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ]),
            _ => None,
        }
    }

    /// Construct an event from its recovered fields without assigning meaning to them.
    #[must_use]
    pub const fn from_fields(
        event_code: u16,
        auxiliary: u16,
        value: u16,
        reserved: [u8; 2],
        special_value: u32,
    ) -> Self {
        Self {
            event_code,
            auxiliary,
            value,
            reserved,
            special_value,
        }
    }

    /// Parse exactly 24 hexadecimal digits into one 12-byte OEM record.
    pub fn parse_record_hex(value: &str) -> Result<[u8; RECORD_LENGTH], InvalidAulaOemMacroRecord> {
        let bytes = value.as_bytes();
        if bytes.len() != RECORD_LENGTH * 2 {
            return Err(InvalidAulaOemMacroRecord::Length);
        }
        let mut record = [0; RECORD_LENGTH];
        for (index, byte) in record.iter_mut().enumerate() {
            let high = hex_nibble(bytes[index * 2]).ok_or(InvalidAulaOemMacroRecord::Hex)?;
            let low = hex_nibble(bytes[index * 2 + 1]).ok_or(InvalidAulaOemMacroRecord::Hex)?;
            *byte = (high << 4) | low;
        }
        Ok(record)
    }

    /// Format one raw record as uppercase hexadecimal for diagnostics and editors.
    #[must_use]
    pub fn format_record_hex(record: [u8; RECORD_LENGTH]) -> String {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut hex = String::with_capacity(RECORD_LENGTH * 2);
        for byte in record {
            hex.push(char::from(HEX[usize::from(byte >> 4)]));
            hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        hex
    }

    /// Decode one fixed-width event row.
    #[must_use]
    pub const fn from_record(record: [u8; RECORD_LENGTH]) -> Self {
        Self {
            event_code: u16::from_le_bytes([record[0], record[1]]),
            auxiliary: u16::from_le_bytes([record[2], record[3]]),
            value: u16::from_le_bytes([record[4], record[5]]),
            reserved: [record[6], record[7]],
            special_value: u32::from_le_bytes([record[8], record[9], record[10], record[11]]),
        }
    }

    /// Encode this event without changing any of its four stored fields.
    #[must_use]
    pub const fn to_record(self) -> [u8; RECORD_LENGTH] {
        let event_code = self.event_code.to_le_bytes();
        let auxiliary = self.auxiliary.to_le_bytes();
        let value = self.value.to_le_bytes();
        let special_value = self.special_value.to_le_bytes();
        [
            event_code[0],
            event_code[1],
            auxiliary[0],
            auxiliary[1],
            value[0],
            value[1],
            self.reserved[0],
            self.reserved[1],
            special_value[0],
            special_value[1],
            special_value[2],
            special_value[3],
        ]
    }

    /// The 16-bit event discriminator stored at bytes 0–1.
    #[must_use]
    pub const fn event_code(self) -> u16 {
        self.event_code
    }

    /// The 16-bit auxiliary field stored at bytes 2–3.
    #[must_use]
    pub const fn auxiliary(self) -> u16 {
        self.auxiliary
    }

    /// The 16-bit value field stored at bytes 4–5.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.value
    }

    /// The two bytes at offsets 6–7, whose meaning is not yet known.
    #[must_use]
    pub const fn reserved(self) -> [u8; 2] {
        self.reserved
    }

    /// The 32-bit special field stored at bytes 8–11.
    #[must_use]
    pub const fn special_value(self) -> u32 {
        self.special_value
    }

    /// Classify the record using observed record-decoding behavior.
    #[must_use]
    pub fn kind(self) -> AulaOemMacroEventKind {
        if self.event_code == 0 && self.value == 0 && self.reserved[0] == 0 && self.reserved[1] == 0
        {
            return AulaOemMacroEventKind::EmptyRow;
        }
        Self::selector_events()
            .iter()
            .find_map(|(code, kind)| (*code == self.event_code).then_some(*kind))
            .unwrap_or(match self.event_code {
                0x0000..=0x00f4 => AulaOemMacroEventKind::MappedInput,
                _ => AulaOemMacroEventKind::Unknown,
            })
    }

    /// Return the OEM macro editor's known selector codes and their labels.
    #[must_use]
    pub const fn selector_events() -> &'static [(u16, AulaOemMacroEventKind)] {
        &SELECTOR_EVENTS
    }
}

impl AulaOemMacroFile {
    /// Construct an empty macro using the fields initialized by the OEM's new-macro path.
    pub fn empty(node_id: u32, name: &str) -> Result<Self, InvalidAulaOemMacroFile> {
        let mut header = [0; HEADER_LENGTH];
        header[0..4].copy_from_slice(&HEADER_MAGIC.to_le_bytes());
        header[4..8].copy_from_slice(&node_id.to_le_bytes());
        header[12..16].copy_from_slice(&1_u32.to_le_bytes());
        header[CREATED_FLAG_OFFSET..CREATED_FLAG_OFFSET + 4].copy_from_slice(&1_u32.to_le_bytes());
        write_name(&mut header, NAME_OFFSET, NAME_LENGTH, name)?;
        Self::new(header, Vec::new())
    }

    /// Decode one `.mcr` file using the installed driver's buffer transform.
    pub fn from_oem_bytes(bytes: &[u8]) -> Result<Self, InvalidAulaOemMacroFile> {
        let mut plaintext = bytes.to_vec();
        if plaintext.len().is_multiple_of(8) {
            tea_transform(&mut plaintext, false);
        }
        Self::from_plaintext(&plaintext)
    }

    /// Decode one unencrypted macro-data block from the local `mac.dct` tree.
    ///
    /// The block is a `0x68`-byte macro header followed by its declared event
    /// rows. This does not parse the surrounding `0x58`-byte tree nodes or
    /// sibling relationships.
    pub fn from_dct_node_bytes(bytes: &[u8]) -> Result<Self, InvalidAulaOemMacroFile> {
        Self::from_plaintext(bytes)
    }

    /// Create an OEM macro while preserving a validated header template.
    pub fn new(
        mut header: [u8; HEADER_LENGTH],
        records: Vec<[u8; RECORD_LENGTH]>,
    ) -> Result<Self, InvalidAulaOemMacroFile> {
        if read_u32(&header, 0) != HEADER_MAGIC {
            return Err(InvalidAulaOemMacroFile::Header);
        }
        if records.len() > AULA_OEM_MACRO_RECORD_LIMIT {
            return Err(InvalidAulaOemMacroFile::RecordLimit);
        }
        let count = u32::try_from(records.len()).map_err(|_| InvalidAulaOemMacroFile::Length)?;
        header[RECORD_COUNT_OFFSET..RECORD_COUNT_OFFSET + 4].copy_from_slice(&count.to_le_bytes());
        Ok(Self { header, records })
    }

    /// Return a copy with one existing raw event replaced.
    pub fn replace_record(
        &self,
        index: usize,
        record: [u8; RECORD_LENGTH],
    ) -> Result<Self, InvalidAulaOemMacroFile> {
        let mut records = self.records.clone();
        let Some(target) = records.get_mut(index) else {
            return Err(InvalidAulaOemMacroFile::RecordIndex);
        };
        *target = record;
        Self::new(self.header, records)
    }

    /// Return a copy with one raw event inserted before `index`.
    pub fn insert_record(
        &self,
        index: usize,
        record: [u8; RECORD_LENGTH],
    ) -> Result<Self, InvalidAulaOemMacroFile> {
        if index > self.records.len() {
            return Err(InvalidAulaOemMacroFile::RecordIndex);
        }
        let mut records = self.records.clone();
        records.insert(index, record);
        Self::new(self.header, records)
    }

    /// Return a copy with one existing raw event removed.
    pub fn remove_record(&self, index: usize) -> Result<Self, InvalidAulaOemMacroFile> {
        if index >= self.records.len() {
            return Err(InvalidAulaOemMacroFile::RecordIndex);
        }
        let mut records = self.records.clone();
        records.remove(index);
        Self::new(self.header, records)
    }

    /// Return a copy with two raw event records exchanged.
    pub fn swap_records(
        &self,
        first_index: usize,
        second_index: usize,
    ) -> Result<Self, InvalidAulaOemMacroFile> {
        if first_index >= self.records.len() || second_index >= self.records.len() {
            return Err(InvalidAulaOemMacroFile::RecordIndex);
        }
        if first_index == second_index {
            return Ok(self.clone());
        }
        let mut records = self.records.clone();
        records.swap(first_index, second_index);
        Self::new(self.header, records)
    }

    /// Return a copy with two visible OEM editor rows exchanged.
    ///
    /// Rows hidden by the OEM editor remain at their original raw indexes, and
    /// the complete 12-byte event records are moved without interpretation.
    pub fn swap_oem_editor_events(
        &self,
        first_visible_index: usize,
        second_visible_index: usize,
    ) -> Result<Self, InvalidAulaOemMacroFile> {
        let first = self
            .oem_editor_record_index(first_visible_index)
            .ok_or(InvalidAulaOemMacroFile::RecordIndex)?;
        let second = self
            .oem_editor_record_index(second_visible_index)
            .ok_or(InvalidAulaOemMacroFile::RecordIndex)?;
        self.swap_records(first, second)
    }

    /// Return a copy with the macro name in its OEM header replaced.
    pub fn rename(&self, name: &str) -> Result<Self, InvalidAulaOemMacroFile> {
        let mut header = self.header;
        write_name(&mut header, NAME_OFFSET, NAME_LENGTH, name)?;
        Self::new(header, self.records.clone())
    }

    /// The optional UTF-16LE macro name recovered from its OEM header.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        read_name(&self.header, NAME_OFFSET, NAME_LENGTH)
    }

    /// The node identifier stored in the OEM macro header at bytes 4–7.
    #[must_use]
    pub fn node_id(&self) -> u32 {
        read_u32(&self.header, 4)
    }

    /// Encode this macro using the installed driver's buffer transform.
    #[must_use]
    pub fn to_oem_bytes(&self) -> Vec<u8> {
        let mut bytes = self.to_plaintext_bytes();
        if bytes.len().is_multiple_of(8) {
            tea_transform(&mut bytes, true);
        }
        bytes
    }

    /// Encode the unencrypted macro-data block stored inside a local `mac.dct`.
    #[must_use]
    pub fn to_dct_node_bytes(&self) -> Vec<u8> {
        self.to_plaintext_bytes()
    }

    /// The preserved OEM macro header, including fields not yet interpreted.
    #[must_use]
    pub const fn header(&self) -> &[u8; HEADER_LENGTH] {
        &self.header
    }

    /// Raw 12-byte events in OEM playback order.
    #[must_use]
    pub fn records(&self) -> &[[u8; RECORD_LENGTH]] {
        &self.records
    }

    /// Events decoded into the recovered fixed-width storage fields.
    pub fn events(&self) -> impl ExactSizeIterator<Item = AulaOemMacroEvent> + '_ {
        self.records
            .iter()
            .copied()
            .map(AulaOemMacroEvent::from_record)
    }

    /// Raw record indexes shown as editable rows by the installed OEM editor.
    pub fn oem_editor_event_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.events().enumerate().filter_map(|(index, event)| {
            (event.kind() != AulaOemMacroEventKind::EmptyRow).then_some(index)
        })
    }

    /// Number of event rows shown by the installed OEM editor.
    #[must_use]
    pub fn oem_editor_event_count(&self) -> usize {
        self.oem_editor_event_indices().count()
    }

    /// Resolve a visible OEM editor row to its preserved raw record index.
    #[must_use]
    pub fn oem_editor_record_index(&self, visible_index: usize) -> Option<usize> {
        self.oem_editor_event_indices().nth(visible_index)
    }

    /// Choose the raw insertion index for a visible OEM editor row position.
    ///
    /// Inserting at the end places the new row after the last visible event and
    /// before any trailing rows hidden by the OEM editor.
    #[must_use]
    pub fn oem_editor_insert_record_index(&self, visible_index: usize) -> usize {
        self.oem_editor_record_index(visible_index)
            .or_else(|| {
                self.oem_editor_event_indices()
                    .last()
                    .map(|index| index.saturating_add(1))
            })
            .unwrap_or(0)
    }

    fn from_plaintext(bytes: &[u8]) -> Result<Self, InvalidAulaOemMacroFile> {
        if bytes.len() < HEADER_LENGTH {
            return Err(InvalidAulaOemMacroFile::Length);
        }
        if read_u32(bytes, 0) != HEADER_MAGIC {
            return Err(InvalidAulaOemMacroFile::Header);
        }
        let count = usize::try_from(read_u32(bytes, RECORD_COUNT_OFFSET))
            .map_err(|_| InvalidAulaOemMacroFile::Length)?;
        if count > AULA_OEM_MACRO_RECORD_LIMIT {
            return Err(InvalidAulaOemMacroFile::RecordLimit);
        }
        let records_length = count
            .checked_mul(RECORD_LENGTH)
            .ok_or(InvalidAulaOemMacroFile::Length)?;
        let expected_length = HEADER_LENGTH
            .checked_add(records_length)
            .ok_or(InvalidAulaOemMacroFile::Length)?;
        if bytes.len() != expected_length {
            return Err(InvalidAulaOemMacroFile::Length);
        }

        let mut header = [0; HEADER_LENGTH];
        header.copy_from_slice(&bytes[..HEADER_LENGTH]);
        let (records, _) = bytes[HEADER_LENGTH..].as_chunks::<RECORD_LENGTH>();
        let records = records.to_vec();
        Ok(Self { header, records })
    }

    fn to_plaintext_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LENGTH + RECORD_LENGTH * self.records.len());
        bytes.extend_from_slice(&self.header);
        for record in &self.records {
            bytes.extend_from_slice(record);
        }
        bytes
    }
}

/// A versioned JSON profile that preserves an OEM macro for offline editing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "UncheckedAulaOemMacroProfile",
    into = "UncheckedAulaOemMacroProfile"
)]
pub struct AulaOemMacroProfile {
    macro_file: AulaOemMacroFile,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedAulaOemMacroProfile {
    format: String,
    version: u8,
    header: Vec<u8>,
    records: Vec<[u8; RECORD_LENGTH]>,
}

impl AulaOemMacroProfile {
    /// Current portable JSON schema version.
    pub const VERSION: u8 = 1;

    /// Wrap an OEM macro file in the portable JSON format.
    #[must_use]
    pub const fn new(macro_file: AulaOemMacroFile) -> Self {
        Self { macro_file }
    }

    /// Decode an OEM `.mcr` file into the portable profile.
    pub fn from_oem_bytes(bytes: &[u8]) -> Result<Self, InvalidAulaOemMacroProfile> {
        AulaOemMacroFile::from_oem_bytes(bytes)
            .map(Self::new)
            .map_err(InvalidAulaOemMacroProfile::MacroFile)
    }

    /// Encode the profile as an OEM `.mcr` file.
    #[must_use]
    pub fn to_oem_bytes(&self) -> Vec<u8> {
        self.macro_file.to_oem_bytes()
    }

    /// The preserved OEM header and raw event records.
    #[must_use]
    pub const fn macro_file(&self) -> &AulaOemMacroFile {
        &self.macro_file
    }
}

impl TryFrom<UncheckedAulaOemMacroProfile> for AulaOemMacroProfile {
    type Error = InvalidAulaOemMacroProfile;

    fn try_from(profile: UncheckedAulaOemMacroProfile) -> Result<Self, Self::Error> {
        if profile.format != AULA_OEM_MACRO_PROFILE_FORMAT {
            return Err(InvalidAulaOemMacroProfile::Format);
        }
        if profile.version != Self::VERSION {
            return Err(InvalidAulaOemMacroProfile::Version);
        }
        let header: [u8; HEADER_LENGTH] = profile
            .header
            .try_into()
            .map_err(|_| InvalidAulaOemMacroProfile::MacroFile(InvalidAulaOemMacroFile::Length))?;
        let declared_count = usize::try_from(read_u32(&header, RECORD_COUNT_OFFSET))
            .map_err(|_| InvalidAulaOemMacroProfile::MacroFile(InvalidAulaOemMacroFile::Length))?;
        if declared_count != profile.records.len() {
            return Err(InvalidAulaOemMacroProfile::MacroFile(
                InvalidAulaOemMacroFile::Length,
            ));
        }
        let macro_file = AulaOemMacroFile::new(header, profile.records)
            .map_err(InvalidAulaOemMacroProfile::MacroFile)?;
        Ok(Self::new(macro_file))
    }
}

impl From<AulaOemMacroProfile> for UncheckedAulaOemMacroProfile {
    fn from(profile: AulaOemMacroProfile) -> Self {
        Self {
            format: AULA_OEM_MACRO_PROFILE_FORMAT.to_owned(),
            version: AulaOemMacroProfile::VERSION,
            header: profile.macro_file.header.to_vec(),
            records: profile.macro_file.records,
        }
    }
}

/// A portable raw AULA macro profile uses an unknown format or invalid version/data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemMacroProfile {
    /// The JSON document uses a different profile format.
    #[error("invalid AULA OEM macro profile format")]
    Format,
    /// The JSON document uses an unsupported schema version.
    #[error("unsupported AULA OEM macro profile version")]
    Version,
    /// The preserved OEM macro is malformed.
    #[error(transparent)]
    MacroFile(#[from] InvalidAulaOemMacroFile),
}

/// An OEM `.mcr` file has an invalid header or event-table length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemMacroFile {
    /// The file length does not match its declared event count.
    #[error("the AULA OEM macro file has an invalid length")]
    Length,
    /// The file does not contain the OEM macro-node magic.
    #[error("the AULA OEM macro file header is invalid")]
    Header,
    /// The file exceeds the installed OEM serializer's record limit.
    #[error("the AULA OEM macro exceeds the 1015-record serializer limit")]
    RecordLimit,
    /// An event replacement, insertion, or removal used an invalid index.
    #[error("the AULA OEM macro event index is out of range")]
    RecordIndex,
    /// A macro name contains more than the OEM constructor's 29 UTF-16 code units.
    #[error("the AULA OEM macro name exceeds 29 UTF-16 code units")]
    NameTooLong,
    /// A macro name contains a NUL code point that would terminate OEM decoding.
    #[error("the AULA OEM macro name contains a NUL character")]
    NameContainsNul,
}

fn write_name(
    header: &mut [u8; HEADER_LENGTH],
    offset: usize,
    length: usize,
    name: &str,
) -> Result<(), InvalidAulaOemMacroFile> {
    if name.contains('\0') {
        return Err(InvalidAulaOemMacroFile::NameContainsNul);
    }
    let units = name.encode_utf16().collect::<Vec<_>>();
    if units.len() > NAME_MAX_UNITS {
        return Err(InvalidAulaOemMacroFile::NameTooLong);
    }
    header[offset..offset + length].fill(0);
    for (chunk, unit) in header[offset..offset + length]
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .zip(units)
    {
        *chunk = unit.to_le_bytes();
    }
    Ok(())
}

fn read_name(header: &[u8; HEADER_LENGTH], offset: usize, length: usize) -> Option<String> {
    let (bytes, _) = header[offset..offset + length].as_chunks::<2>();
    let units = bytes
        .iter()
        .map(|[low, high]| u16::from_le_bytes([*low, *high]))
        .take_while(|unit| *unit != 0)
        .collect::<Vec<_>>();
    if units.is_empty() {
        return None;
    }
    Some(String::from_utf16_lossy(&units))
}

/// A raw AULA macro event record is not exactly 24 hexadecimal digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemMacroRecord {
    /// The record does not contain exactly 12 bytes.
    #[error("an AULA macro event requires exactly 24 hexadecimal digits")]
    Length,
    /// The record contains a non-hexadecimal character.
    #[error("an AULA macro event contains a non-hexadecimal character")]
    Hex,
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn tea_transform(bytes: &mut [u8], encrypt: bool) {
    if !bytes.len().is_multiple_of(8) {
        return;
    }
    let (blocks, _) = bytes.as_chunks_mut::<8>();
    for block in blocks {
        let mut first = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        let mut second = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        if encrypt {
            let mut sum = 0_u32;
            for _ in 0..32 {
                sum = sum.wrapping_add(TEA_DELTA);
                first = first.wrapping_add(tea_mix(second, TEA_KEY[0], TEA_KEY[1], sum));
                second = second.wrapping_add(tea_mix(first, TEA_KEY[2], TEA_KEY[3], sum));
            }
        } else {
            let mut sum = TEA_DELTA.wrapping_mul(32);
            for _ in 0..32 {
                second = second.wrapping_sub(tea_mix(first, TEA_KEY[2], TEA_KEY[3], sum));
                first = first.wrapping_sub(tea_mix(second, TEA_KEY[0], TEA_KEY[1], sum));
                sum = sum.wrapping_sub(TEA_DELTA);
            }
        }
        block[..4].copy_from_slice(&first.to_le_bytes());
        block[4..].copy_from_slice(&second.to_le_bytes());
    }
}

fn tea_mix(value: u32, first_key: u32, second_key: u32, sum: u32) -> u32 {
    value.wrapping_shl(4).wrapping_add(first_key)
        ^ value.wrapping_add(sum)
        ^ (value >> 5).wrapping_add(second_key)
}
