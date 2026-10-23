//! Host-free codec for the installed AULA `mac.dct` macro tree.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{AulaOemMacroFile, AulaOemMacroProfile, InvalidAulaOemMacroFile};

const ROOT_HEADER_LENGTH: usize = 0x58;
const ROOT_MAGIC: u32 = 0xa8b8_3333;
/// Parent identifier used by the OEM macro tree for top-level nodes.
pub const AULA_OEM_MACRO_TREE_ROOT_NODE_ID: u32 = 0xffff_0100;
const MACRO_FLAG_OFFSET: usize = 0x14;
const NODE_NAME_OFFSET: usize = 0x18;
const NODE_NAME_LENGTH: usize = ROOT_HEADER_LENGTH - NODE_NAME_OFFSET;
const NODE_NAME_MAX_UNITS: usize = 29;
const MACRO_HEADER_LENGTH: usize = 0x68;
const MACRO_COUNT_OFFSET: usize = 0x58;
const EVENT_LENGTH: usize = 12;

/// Stable format identifier for portable raw AULA macro-tree profiles.
pub const AULA_OEM_MACRO_TREE_PROFILE_FORMAT: &str = "openhid-aula-oem-macro-tree";

/// One raw node in the local AULA macro library, preserving its OEM header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaOemMacroTreeNode {
    header: [u8; ROOT_HEADER_LENGTH],
    macro_file: Option<AulaOemMacroFile>,
}

impl AulaOemMacroTreeNode {
    /// Build a node from its exact OEM header and optional macro block.
    pub fn new(
        header: [u8; ROOT_HEADER_LENGTH],
        macro_file: Option<AulaOemMacroFile>,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        if read_u32(&header, 0) != ROOT_MAGIC {
            return Err(InvalidAulaOemMacroTree::Header);
        }
        if (read_u32(&header, MACRO_FLAG_OFFSET) != 0) != macro_file.is_some() {
            return Err(InvalidAulaOemMacroTree::MacroPresence);
        }
        if macro_file
            .as_ref()
            .is_some_and(|macro_file| macro_file.node_id() != read_u32(&header, 4))
        {
            return Err(InvalidAulaOemMacroTree::MacroNodeIdMismatch);
        }
        Ok(Self { header, macro_file })
    }

    /// The preserved 0x58-byte node header.
    #[must_use]
    pub const fn header(&self) -> &[u8; ROOT_HEADER_LENGTH] {
        &self.header
    }

    /// The node identifier stored at header bytes 4–7.
    #[must_use]
    pub fn node_id(&self) -> u32 {
        read_u32(&self.header, 4)
    }

    /// The parent identifier stored at header bytes 8–11.
    #[must_use]
    pub fn parent_id(&self) -> u32 {
        read_u32(&self.header, 8)
    }

    /// The raw node-kind value stored at header bytes 12–15.
    #[must_use]
    pub fn kind_code(&self) -> u32 {
        read_u32(&self.header, 12)
    }

    /// The optional node name stored as a fixed-width UTF-16LE string.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        if let Some(macro_file) = &self.macro_file {
            return macro_file.name();
        }
        let (bytes, _) =
            self.header[NODE_NAME_OFFSET..NODE_NAME_OFFSET + NODE_NAME_LENGTH].as_chunks::<2>();
        let units = bytes
            .iter()
            .map(|[low, high]| u16::from_le_bytes([*low, *high]))
            .take_while(|unit| *unit != 0)
            .collect::<Vec<_>>();
        if units.is_empty() {
            self.macro_file.as_ref().and_then(AulaOemMacroFile::name)
        } else {
            Some(String::from_utf16_lossy(&units))
        }
    }

    fn renamed(&self, name: &str) -> Result<Self, InvalidAulaOemMacroTree> {
        if let Some(macro_file) = &self.macro_file {
            let mut node = self.clone();
            node.macro_file = Some(macro_file.rename(name)?);
            return Ok(node);
        }
        if name.contains('\0') {
            return Err(InvalidAulaOemMacroTree::NodeNameContainsNul);
        }
        let units = name.encode_utf16().collect::<Vec<_>>();
        if units.len() > NODE_NAME_MAX_UNITS {
            return Err(InvalidAulaOemMacroTree::NodeNameTooLong);
        }
        let mut node = self.clone();
        node.header[NODE_NAME_OFFSET..].fill(0);
        for (chunk, unit) in node.header[NODE_NAME_OFFSET..]
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(units)
        {
            *chunk = unit.to_le_bytes();
        }
        Ok(node)
    }

    /// The optional macro block and its raw events.
    #[must_use]
    pub const fn macro_file(&self) -> Option<&AulaOemMacroFile> {
        self.macro_file.as_ref()
    }
}

/// The ordered, lossless node sequence stored in a local `mac.dct` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaOemMacroTree {
    nodes: Vec<AulaOemMacroTreeNode>,
}

impl AulaOemMacroTree {
    /// Decode the unencrypted concatenated nodes in a local `mac.dct` file.
    pub fn from_dct_bytes(bytes: &[u8]) -> Result<Self, InvalidAulaOemMacroTree> {
        let mut nodes = Vec::new();
        let mut offset = 0_usize;

        while offset < bytes.len() {
            let root_end = offset
                .checked_add(ROOT_HEADER_LENGTH)
                .ok_or(InvalidAulaOemMacroTree::Length)?;
            let root_bytes = bytes
                .get(offset..root_end)
                .ok_or(InvalidAulaOemMacroTree::Length)?;
            let mut header = [0; ROOT_HEADER_LENGTH];
            header.copy_from_slice(root_bytes);
            if read_u32(&header, 0) != ROOT_MAGIC {
                return Err(InvalidAulaOemMacroTree::Header);
            }
            offset = root_end;

            let macro_file = if read_u32(&header, MACRO_FLAG_OFFSET) == 0 {
                None
            } else {
                let macro_header_end = offset
                    .checked_add(MACRO_HEADER_LENGTH)
                    .ok_or(InvalidAulaOemMacroTree::Length)?;
                let macro_header = bytes
                    .get(offset..macro_header_end)
                    .ok_or(InvalidAulaOemMacroTree::Length)?;
                let count = usize::try_from(read_u32(macro_header, MACRO_COUNT_OFFSET))
                    .map_err(|_| InvalidAulaOemMacroTree::Length)?;
                let events_length = count
                    .checked_mul(EVENT_LENGTH)
                    .ok_or(InvalidAulaOemMacroTree::Length)?;
                let macro_end = macro_header_end
                    .checked_add(events_length)
                    .ok_or(InvalidAulaOemMacroTree::Length)?;
                let macro_bytes = bytes
                    .get(offset..macro_end)
                    .ok_or(InvalidAulaOemMacroTree::Length)?;
                let macro_file = AulaOemMacroFile::from_dct_node_bytes(macro_bytes)
                    .map_err(InvalidAulaOemMacroTree::MacroFile)?;
                offset = macro_end;
                Some(macro_file)
            };

            nodes.push(AulaOemMacroTreeNode::new(header, macro_file)?);
        }

        Self::new(nodes)
    }

    /// Build a tree from validated OEM nodes in file order.
    ///
    /// Node IDs must be unique because the OEM resolves parents by ID. Parent
    /// chains must be acyclic, and the root sentinel is not a serialized node ID.
    pub fn new(nodes: Vec<AulaOemMacroTreeNode>) -> Result<Self, InvalidAulaOemMacroTree> {
        let mut node_ids = HashSet::with_capacity(nodes.len());
        let mut nodes_by_id = HashMap::with_capacity(nodes.len());
        for node in &nodes {
            let node_id = node.node_id();
            if node_id == AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
                return Err(InvalidAulaOemMacroTree::ReservedNodeId);
            }
            if !node_ids.insert(node_id) {
                return Err(InvalidAulaOemMacroTree::DuplicateNodeId);
            }
            nodes_by_id.insert(node_id, node);
        }

        let mut resolved = HashSet::with_capacity(nodes.len());
        for node in &nodes {
            let mut current_id = node.node_id();
            let mut path = Vec::new();
            let mut path_ids = HashSet::new();
            loop {
                if resolved.contains(&current_id) {
                    break;
                }
                if !path_ids.insert(current_id) {
                    return Err(InvalidAulaOemMacroTree::ParentCycle);
                }
                path.push(current_id);
                let Some(node) = nodes_by_id.get(&current_id) else {
                    break;
                };
                let parent_id = node.parent_id();
                if parent_id == AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
                    break;
                }
                let Some(parent) = nodes_by_id.get(&parent_id) else {
                    break;
                };
                if parent.kind_code() != 1 || parent.macro_file.is_some() {
                    return Err(InvalidAulaOemMacroTree::ParentNotFolder);
                }
                current_id = parent_id;
            }
            resolved.extend(path);
        }
        Ok(Self { nodes })
    }

    /// Encode the nodes as the unencrypted local `mac.dct` representation.
    #[must_use]
    pub fn to_dct_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for node in &self.nodes {
            bytes.extend_from_slice(&node.header);
            if let Some(macro_file) = &node.macro_file {
                bytes.extend_from_slice(&macro_file.to_dct_node_bytes());
            }
        }
        bytes
    }

    /// Nodes in their original file order.
    #[must_use]
    pub fn nodes(&self) -> &[AulaOemMacroTreeNode] {
        &self.nodes
    }

    /// Resolve a node's root-to-node path using the OEM parent identifiers.
    ///
    /// Missing parent records leave an orphan path rooted at the first available
    /// ancestor. A cyclic parent chain is rejected.
    pub fn node_path_indices(
        &self,
        node_index: usize,
    ) -> Result<Vec<usize>, InvalidAulaOemMacroTree> {
        let mut path = Vec::new();
        let mut current = Some(node_index);
        while let Some(index) = current {
            let node = self
                .nodes
                .get(index)
                .ok_or(InvalidAulaOemMacroTree::NodeIndex)?;
            if path.contains(&index) {
                return Err(InvalidAulaOemMacroTree::ParentCycle);
            }
            path.push(index);
            let parent_id = node.parent_id();
            current = if parent_id == AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
                None
            } else {
                self.nodes
                    .iter()
                    .position(|candidate| candidate.node_id() == parent_id)
            };
        }
        path.reverse();
        Ok(path)
    }

    /// Return a copy with one node's UTF-16LE name replaced.
    pub fn rename_node(
        &self,
        node_index: usize,
        name: &str,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        let mut nodes = self.nodes.clone();
        let node = nodes
            .get_mut(node_index)
            .ok_or(InvalidAulaOemMacroTree::NodeIndex)?;
        *node = node.renamed(name)?;
        Ok(Self { nodes })
    }

    /// Return a copy with a new empty OEM folder attached to a root or folder node.
    ///
    /// `node_id` is a local tree identifier. It must be unique within this tree.
    pub fn create_folder(
        &self,
        parent_id: u32,
        node_id: u32,
        name: &str,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        if node_id == AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
            return Err(InvalidAulaOemMacroTree::ReservedNodeId);
        }
        if self.nodes.iter().any(|node| node.node_id() == node_id) {
            return Err(InvalidAulaOemMacroTree::DuplicateNodeId);
        }
        if parent_id != AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
            let parent = self
                .nodes
                .iter()
                .find(|node| node.node_id() == parent_id)
                .ok_or(InvalidAulaOemMacroTree::ParentMissing)?;
            if parent.kind_code() != 1 || parent.macro_file.is_some() {
                return Err(InvalidAulaOemMacroTree::ParentNotFolder);
            }
        }

        let mut header = [0; ROOT_HEADER_LENGTH];
        header[0..4].copy_from_slice(&ROOT_MAGIC.to_le_bytes());
        header[4..8].copy_from_slice(&node_id.to_le_bytes());
        header[8..12].copy_from_slice(&parent_id.to_le_bytes());
        header[12..16].copy_from_slice(&1_u32.to_le_bytes());
        let folder = AulaOemMacroTreeNode::new(header, None)?.renamed(name)?;
        let mut nodes = self.nodes.clone();
        nodes.push(folder);
        Ok(Self { nodes })
    }

    /// Return a copy with a new empty OEM macro attached to a root or folder node.
    ///
    /// `node_id` is stored in both the tree node and macro header. It must be
    /// unique within this tree.
    pub fn create_macro(
        &self,
        parent_id: u32,
        node_id: u32,
        name: &str,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        if node_id == AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
            return Err(InvalidAulaOemMacroTree::ReservedNodeId);
        }
        if self.nodes.iter().any(|node| node.node_id() == node_id) {
            return Err(InvalidAulaOemMacroTree::DuplicateNodeId);
        }
        if parent_id != AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
            let parent = self
                .nodes
                .iter()
                .find(|node| node.node_id() == parent_id)
                .ok_or(InvalidAulaOemMacroTree::ParentMissing)?;
            if parent.kind_code() != 1 || parent.macro_file.is_some() {
                return Err(InvalidAulaOemMacroTree::ParentNotFolder);
            }
        }

        let mut header = [0; ROOT_HEADER_LENGTH];
        header[0..4].copy_from_slice(&ROOT_MAGIC.to_le_bytes());
        header[4..8].copy_from_slice(&node_id.to_le_bytes());
        header[8..12].copy_from_slice(&parent_id.to_le_bytes());
        header[MACRO_FLAG_OFFSET..MACRO_FLAG_OFFSET + 4].copy_from_slice(&1_u32.to_le_bytes());
        let macro_file = AulaOemMacroFile::empty(node_id, name)?;
        let macro_node = AulaOemMacroTreeNode::new(header, Some(macro_file))?;
        let mut nodes = self.nodes.clone();
        nodes.push(macro_node);
        Ok(Self { nodes })
    }

    /// Resolve a candidate identifier to the next ID unused by this tree.
    ///
    /// The root sentinel is reserved. On collision, candidates advance with
    /// wrapping arithmetic until an available local identifier is found.
    #[must_use]
    pub fn next_available_node_id(&self, mut candidate: u32) -> u32 {
        while candidate == AULA_OEM_MACRO_TREE_ROOT_NODE_ID
            || self.nodes.iter().any(|node| node.node_id() == candidate)
        {
            candidate = candidate.wrapping_add(1);
        }
        candidate
    }

    /// Return a copy with one leaf node removed.
    ///
    /// Folders with children are rejected so this operation never deletes a
    /// subtree implicitly. `node_index` addresses the node order from `nodes()`.
    pub fn remove_node(&self, node_index: usize) -> Result<Self, InvalidAulaOemMacroTree> {
        let node = self
            .nodes
            .get(node_index)
            .ok_or(InvalidAulaOemMacroTree::NodeIndex)?;
        if self
            .nodes
            .iter()
            .any(|candidate| candidate.parent_id() == node.node_id())
        {
            return Err(InvalidAulaOemMacroTree::NodeHasChildren);
        }
        let mut nodes = self.nodes.clone();
        nodes.remove(node_index);
        Ok(Self { nodes })
    }

    /// Return a copy with one event in a node's macro block replaced.
    pub fn replace_event(
        &self,
        node_index: usize,
        event_index: usize,
        record: [u8; EVENT_LENGTH],
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        self.edit_event(node_index, |macro_file| {
            macro_file.replace_record(event_index, record)
        })
    }

    /// Return a copy with one event inserted before the given macro event index.
    pub fn insert_event(
        &self,
        node_index: usize,
        event_index: usize,
        record: [u8; EVENT_LENGTH],
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        self.edit_event(node_index, |macro_file| {
            macro_file.insert_record(event_index, record)
        })
    }

    /// Return a copy with one event removed from a node's macro block.
    pub fn remove_event(
        &self,
        node_index: usize,
        event_index: usize,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        self.edit_event(node_index, |macro_file| {
            macro_file.remove_record(event_index)
        })
    }

    /// Return a copy with two raw events in a macro-tree node exchanged.
    pub fn swap_events(
        &self,
        node_index: usize,
        first_event_index: usize,
        second_event_index: usize,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        self.edit_event(node_index, |macro_file| {
            macro_file.swap_records(first_event_index, second_event_index)
        })
    }

    /// Replace one node's macro block while preserving every node header.
    pub fn replace_macro_file(
        &self,
        node_index: usize,
        macro_file: AulaOemMacroFile,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        let mut nodes = self.nodes.clone();
        let node = nodes
            .get_mut(node_index)
            .ok_or(InvalidAulaOemMacroTree::NodeIndex)?;
        if node.macro_file.is_none() {
            return Err(InvalidAulaOemMacroTree::MissingMacro);
        }
        node.macro_file = Some(macro_file);
        Ok(Self { nodes })
    }

    fn edit_event(
        &self,
        node_index: usize,
        edit: impl FnOnce(&AulaOemMacroFile) -> Result<AulaOemMacroFile, InvalidAulaOemMacroFile>,
    ) -> Result<Self, InvalidAulaOemMacroTree> {
        let mut nodes = self.nodes.clone();
        let node = nodes
            .get_mut(node_index)
            .ok_or(InvalidAulaOemMacroTree::NodeIndex)?;
        let macro_file = node
            .macro_file
            .as_ref()
            .ok_or(InvalidAulaOemMacroTree::MissingMacro)?;
        node.macro_file = Some(edit(macro_file).map_err(InvalidAulaOemMacroTree::MacroFile)?);
        Ok(Self { nodes })
    }
}

/// A versioned JSON profile preserving every node and macro event in `mac.dct`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "UncheckedAulaOemMacroTreeProfile",
    into = "UncheckedAulaOemMacroTreeProfile"
)]
pub struct AulaOemMacroTreeProfile {
    tree: AulaOemMacroTree,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedAulaOemMacroTreeProfile {
    format: String,
    version: u8,
    nodes: Vec<UncheckedAulaOemMacroTreeNode>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedAulaOemMacroTreeNode {
    header: Vec<u8>,
    macro_profile: Option<AulaOemMacroProfile>,
}

impl AulaOemMacroTreeProfile {
    /// Current portable JSON schema version.
    pub const VERSION: u8 = 1;

    /// Wrap a validated macro tree in the portable profile format.
    #[must_use]
    pub const fn new(tree: AulaOemMacroTree) -> Self {
        Self { tree }
    }

    /// Decode a local `mac.dct` file into the portable profile.
    pub fn from_dct_bytes(bytes: &[u8]) -> Result<Self, InvalidAulaOemMacroTreeProfile> {
        AulaOemMacroTree::from_dct_bytes(bytes)
            .map(Self::new)
            .map_err(InvalidAulaOemMacroTreeProfile::Tree)
    }

    /// Encode the profile as a local `mac.dct` file.
    #[must_use]
    pub fn to_dct_bytes(&self) -> Vec<u8> {
        self.tree.to_dct_bytes()
    }

    /// The preserved macro tree.
    #[must_use]
    pub const fn tree(&self) -> &AulaOemMacroTree {
        &self.tree
    }
}

impl TryFrom<UncheckedAulaOemMacroTreeProfile> for AulaOemMacroTreeProfile {
    type Error = InvalidAulaOemMacroTreeProfile;

    fn try_from(profile: UncheckedAulaOemMacroTreeProfile) -> Result<Self, Self::Error> {
        if profile.format != AULA_OEM_MACRO_TREE_PROFILE_FORMAT {
            return Err(InvalidAulaOemMacroTreeProfile::Format);
        }
        if profile.version != Self::VERSION {
            return Err(InvalidAulaOemMacroTreeProfile::Version);
        }
        let nodes = profile
            .nodes
            .into_iter()
            .map(|node| {
                let header: [u8; ROOT_HEADER_LENGTH] = node.header.try_into().map_err(|_| {
                    InvalidAulaOemMacroTreeProfile::Tree(InvalidAulaOemMacroTree::Length)
                })?;
                AulaOemMacroTreeNode::new(
                    header,
                    node.macro_profile
                        .map(|profile| profile.macro_file().clone()),
                )
                .map_err(InvalidAulaOemMacroTreeProfile::Tree)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let tree = AulaOemMacroTree::new(nodes).map_err(InvalidAulaOemMacroTreeProfile::Tree)?;
        Ok(Self::new(tree))
    }
}

impl From<AulaOemMacroTreeProfile> for UncheckedAulaOemMacroTreeProfile {
    fn from(profile: AulaOemMacroTreeProfile) -> Self {
        Self {
            format: AULA_OEM_MACRO_TREE_PROFILE_FORMAT.to_owned(),
            version: AulaOemMacroTreeProfile::VERSION,
            nodes: profile
                .tree
                .nodes
                .into_iter()
                .map(|node| UncheckedAulaOemMacroTreeNode {
                    header: node.header.to_vec(),
                    macro_profile: node.macro_file.map(AulaOemMacroProfile::new),
                })
                .collect(),
        }
    }
}

/// A local AULA macro tree is malformed or uses an unsupported node layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemMacroTree {
    /// A node or its macro block is truncated or has an inconsistent length.
    #[error("the AULA OEM macro tree has an invalid length")]
    Length,
    /// A node does not contain the expected root magic.
    #[error("the AULA OEM macro tree node header is invalid")]
    Header,
    /// The node's macro-presence marker disagrees with its macro block.
    #[error("the AULA OEM macro tree node has inconsistent macro data")]
    MacroPresence,
    /// The macro block's node ID disagrees with its containing tree-node ID.
    #[error("the AULA macro block ID does not match its tree node ID")]
    MacroNodeIdMismatch,
    /// An operation addressed a node outside the tree.
    #[error("the AULA OEM macro tree node index is out of range")]
    NodeIndex,
    /// More than one node uses the same identifier.
    #[error("the AULA macro-tree node identifier is already in use")]
    DuplicateNodeId,
    /// A serialized node uses the root sentinel reserved for parent references.
    #[error("the AULA macro-tree root identifier is reserved")]
    ReservedNodeId,
    /// A new folder references a parent identifier absent from the tree.
    #[error("the AULA macro-tree parent does not exist")]
    ParentMissing,
    /// A node's known parent is a macro or another unsupported node kind.
    #[error("the AULA macro-tree parent is not a folder")]
    ParentNotFolder,
    /// A folder cannot be removed while it contains child nodes.
    #[error("the AULA macro-tree node still has children")]
    NodeHasChildren,
    /// A node's parent identifiers form a cycle.
    #[error("the AULA OEM macro tree contains a cyclic parent chain")]
    ParentCycle,
    /// A node name contains more than the OEM constructor's 29 UTF-16 code units.
    #[error("the AULA OEM macro node name exceeds 29 UTF-16 code units")]
    NodeNameTooLong,
    /// A node name contains a NUL code point that would terminate OEM decoding.
    #[error("the AULA OEM macro node name contains a NUL character")]
    NodeNameContainsNul,
    /// An operation addressed a node without a macro block.
    #[error("the AULA OEM macro tree node has no macro data")]
    MissingMacro,
    /// The macro block is malformed.
    #[error(transparent)]
    MacroFile(#[from] InvalidAulaOemMacroFile),
}

/// A portable AULA macro-tree profile uses an unknown format or invalid data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InvalidAulaOemMacroTreeProfile {
    /// The JSON document uses a different profile format.
    #[error("invalid AULA OEM macro-tree profile format")]
    Format,
    /// The JSON document uses an unsupported schema version.
    #[error("unsupported AULA OEM macro-tree profile version")]
    Version,
    /// A node or macro block is malformed.
    #[error(transparent)]
    Tree(#[from] InvalidAulaOemMacroTree),
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
