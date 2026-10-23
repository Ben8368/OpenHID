//! Offline conversion and editing for AULA OEM macro files and portable profiles.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use clap::Subcommand;
use openhid_core::hid::aula::{
    AulaOemMacroEvent, AulaOemMacroFile, AulaOemMacroProfile, AulaOemMacroTreeProfile,
    aula_oem_macro_transfer_block, aula_oem_macro_transfer_table,
};

/// Offline AULA macro profile conversion commands.
#[derive(Debug, Subcommand)]
pub enum AulaMacroCmd {
    /// Convert an OEM `.mcr` file to a versioned OpenHID JSON profile.
    Import {
        /// Source OEM macro file.
        #[arg(long)]
        input: PathBuf,
        /// New JSON profile path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Convert a versioned OpenHID JSON profile to an OEM `.mcr` file.
    Export {
        /// Source OpenHID macro profile.
        #[arg(long)]
        input: PathBuf,
        /// New OEM macro file path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Print classified event codes and raw fields from an OEM file or profile.
    Inspect {
        /// OEM `.mcr` or versioned OpenHID macro profile.
        #[arg(long)]
        input: PathBuf,
    },
    /// Replace one event with an exact 12-byte raw record, preserving every other field.
    ReplaceEvent {
        /// OEM `.mcr` or versioned OpenHID macro profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.mcr` or JSON profile path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based event index from `inspect`.
        #[arg(long)]
        index: usize,
        /// Exactly 24 hexadecimal digits for one raw event record.
        #[arg(long)]
        record: String,
    },
    /// Insert one exact 12-byte raw event into an OEM macro or JSON profile.
    InsertEvent {
        /// OEM `.mcr` or versioned OpenHID macro profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.mcr` or JSON profile path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based insertion index, from 0 through the current event count.
        #[arg(long)]
        index: usize,
        /// Exactly 24 hexadecimal digits for one raw event record.
        #[arg(long)]
        record: String,
    },
    /// Remove one raw event from an OEM macro or JSON profile.
    RemoveEvent {
        /// OEM `.mcr` or versioned OpenHID macro profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.mcr` or JSON profile path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based event index from `inspect`.
        #[arg(long)]
        index: usize,
    },
    /// Swap two raw event records in an OEM macro or JSON profile.
    SwapEvents {
        /// OEM `.mcr` or versioned OpenHID macro profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.mcr` or JSON profile path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// First zero-based raw event index from `inspect`.
        #[arg(long)]
        first_index: usize,
        /// Second zero-based raw event index from `inspect`.
        #[arg(long)]
        second_index: usize,
    },
    /// Encode one local macro using the reconstructed OEM per-node transfer format.
    EncodeTransferBlock {
        /// OEM `.mcr` or versioned OpenHID macro profile.
        #[arg(long)]
        input: PathBuf,
        /// New raw transfer-block path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Inspect, convert and edit a local `mac.dct` macro tree.
    Tree {
        #[command(subcommand)]
        command: AulaMacroTreeCmd,
    },
}

/// Commands that operate on a local AULA `mac.dct` tree.
#[derive(Debug, Subcommand)]
pub enum AulaMacroTreeCmd {
    /// Convert a local, unencrypted `mac.dct` tree to a portable JSON profile.
    Import {
        /// Source local macro tree.
        #[arg(long)]
        input: PathBuf,
        /// New JSON tree profile path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Convert a portable JSON tree profile to a local `mac.dct` tree.
    Export {
        /// Source OpenHID macro-tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New local macro tree path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Print raw node headers and event counts from a `mac.dct` tree or profile.
    Inspect {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
    },
    /// Rename one folder or macro node while preserving its other header bytes.
    RenameNode {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
        /// New name, limited to 29 UTF-16 code units by the OEM format.
        #[arg(long)]
        name: String,
    },
    /// Add an empty OEM folder under the root or an existing folder.
    CreateFolder {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Parent node identifier in hexadecimal (for example `0xFFFF0100` for root).
        #[arg(long, value_parser = parse_node_id)]
        parent_id: u32,
        /// New folder name, limited to 29 UTF-16 code units.
        #[arg(long)]
        name: String,
    },
    /// Add an empty OEM macro under the root or an existing folder.
    CreateMacro {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Parent node identifier in hexadecimal (for example `0xFFFF0100` for root).
        #[arg(long, value_parser = parse_node_id)]
        parent_id: u32,
        /// New macro name, limited to 29 UTF-16 code units.
        #[arg(long)]
        name: String,
    },
    /// Remove one leaf folder or macro node from a local macro tree.
    DeleteNode {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
    },
    /// Extract one node's macro as an encrypted OEM `.mcr` file.
    ExtractMacro {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// Zero-based node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
        /// New `.mcr` output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Encode one tree macro node using the reconstructed OEM per-node format.
    EncodeTransferBlock {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// Zero-based macro-node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
        /// New raw transfer-block path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Assemble an OEM macro transfer table from nodes in first-reference order.
    EncodeTransferTable {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// Macro-node indices in the order their keys first reference them.
        #[arg(long = "node-index", required = true, num_args = 1..)]
        node_indices: Vec<usize>,
        /// New raw transfer-table path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Replace one raw event in a macro-tree node.
    ReplaceEvent {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
        /// Zero-based event index from `tree inspect`.
        #[arg(long)]
        event_index: usize,
        /// Exactly 24 hexadecimal digits for one raw event record.
        #[arg(long)]
        record: String,
    },
    /// Insert one raw event in a macro-tree node.
    InsertEvent {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
        /// Zero-based insertion index, from 0 through the current event count.
        #[arg(long)]
        event_index: usize,
        /// Exactly 24 hexadecimal digits for one raw event record.
        #[arg(long)]
        record: String,
    },
    /// Remove one raw event from a macro-tree node.
    RemoveEvent {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
        /// Zero-based event index from `tree inspect`.
        #[arg(long)]
        event_index: usize,
    },
    /// Swap two raw event records in a macro-tree node.
    SwapEvents {
        /// Local `mac.dct` or portable OpenHID tree profile.
        #[arg(long)]
        input: PathBuf,
        /// New `.dct` or JSON output path; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
        /// Zero-based node index from `tree inspect`.
        #[arg(long)]
        node_index: usize,
        /// First zero-based raw event index from `tree inspect`.
        #[arg(long)]
        first_event_index: usize,
        /// Second zero-based raw event index from `tree inspect`.
        #[arg(long)]
        second_event_index: usize,
    },
}

impl AulaMacroCmd {
    /// Run the requested file conversion without opening HID devices.
    pub fn run(self) -> Result<()> {
        match self {
            Self::Import { input, output } => {
                let bytes = std::fs::read(&input)
                    .with_context(|| format!("read OEM macro file {}", input.display()))?;
                let profile = AulaOemMacroProfile::from_oem_bytes(&bytes)
                    .context("invalid or unsupported AULA OEM macro file")?;
                write_new_file(&output, &serde_json::to_vec_pretty(&profile)?)?;
                println!(
                    "Imported {} raw macro events to {}. Event meanings are preserved as raw data.",
                    profile.macro_file().records().len(),
                    output.display()
                );
            }
            Self::Export { input, output } => {
                let bytes = std::fs::read(&input)
                    .with_context(|| format!("read AULA macro profile {}", input.display()))?;
                let profile = serde_json::from_slice::<AulaOemMacroProfile>(&bytes)
                    .context("invalid or unsupported AULA macro profile")?;
                write_new_file(&output, &profile.to_oem_bytes())?;
                println!(
                    "Exported {} raw macro events to {}.",
                    profile.macro_file().records().len(),
                    output.display()
                );
            }
            Self::Inspect { input } => {
                let profile = read_profile(&input)?;
                let macro_file = profile.macro_file();
                println!(
                    "{} raw records ({} rows shown by the OEM editor)",
                    macro_file.records().len(),
                    macro_file.oem_editor_event_count()
                );
                for (index, event) in macro_file.events().enumerate() {
                    let record = AulaOemMacroEvent::format_record_hex(macro_file.records()[index]);
                    let reserved = event.reserved();
                    println!(
                        "{index:04} {} code=0x{:04X} auxiliary=0x{:04X} value=0x{:04X} reserved={:02X}{:02X} special=0x{:08X} record={record}",
                        event.kind(),
                        event.event_code(),
                        event.auxiliary(),
                        event.value(),
                        reserved[0],
                        reserved[1],
                        event.special_value(),
                    );
                }
            }
            Self::ReplaceEvent {
                input,
                output,
                index,
                record,
            } => replace_event(&input, &output, index, &record)?,
            Self::InsertEvent {
                input,
                output,
                index,
                record,
            } => insert_event(&input, &output, index, &record)?,
            Self::RemoveEvent {
                input,
                output,
                index,
            } => remove_event(&input, &output, index)?,
            Self::SwapEvents {
                input,
                output,
                first_index,
                second_index,
            } => swap_events(&input, &output, first_index, second_index)?,
            Self::EncodeTransferBlock { input, output } => {
                encode_macro_file_transfer_block(&input, &output)?;
            }
            Self::Tree { command } => command.run()?,
        }
        Ok(())
    }
}

impl AulaMacroTreeCmd {
    fn run(self) -> Result<()> {
        match self {
            Self::Import { input, output } => import_tree(&input, &output)?,
            Self::Export { input, output } => export_tree(&input, &output)?,
            Self::Inspect { input } => inspect_tree(&input)?,
            Self::RenameNode {
                input,
                output,
                node_index,
                name,
            } => rename_tree_node(&input, &output, node_index, &name)?,
            Self::CreateFolder {
                input,
                output,
                parent_id,
                name,
            } => create_tree_folder(&input, &output, parent_id, &name)?,
            Self::CreateMacro {
                input,
                output,
                parent_id,
                name,
            } => create_tree_macro(&input, &output, parent_id, &name)?,
            Self::DeleteNode {
                input,
                output,
                node_index,
            } => delete_tree_node(&input, &output, node_index)?,
            Self::ExtractMacro {
                input,
                node_index,
                output,
            } => extract_tree_macro(&input, node_index, &output)?,
            Self::EncodeTransferBlock {
                input,
                node_index,
                output,
            } => encode_tree_transfer_block(&input, node_index, &output)?,
            Self::EncodeTransferTable {
                input,
                node_indices,
                output,
            } => encode_tree_transfer_table(&input, &node_indices, &output)?,
            Self::ReplaceEvent {
                input,
                output,
                node_index,
                event_index,
                record,
            } => edit_tree_event(
                &input,
                &output,
                node_index,
                event_index,
                Some(&record),
                false,
            )?,
            Self::InsertEvent {
                input,
                output,
                node_index,
                event_index,
                record,
            } => edit_tree_event(
                &input,
                &output,
                node_index,
                event_index,
                Some(&record),
                true,
            )?,
            Self::RemoveEvent {
                input,
                output,
                node_index,
                event_index,
            } => edit_tree_event(&input, &output, node_index, event_index, None, false)?,
            Self::SwapEvents {
                input,
                output,
                node_index,
                first_event_index,
                second_event_index,
            } => swap_tree_events(
                &input,
                &output,
                node_index,
                first_event_index,
                second_event_index,
            )?,
        }
        Ok(())
    }
}

fn import_tree(input: &Path, output: &Path) -> Result<()> {
    let bytes = std::fs::read(input)
        .with_context(|| format!("read AULA macro tree {}", input.display()))?;
    let profile = AulaOemMacroTreeProfile::from_dct_bytes(&bytes)
        .context("invalid or unsupported AULA local macro tree")?;
    let node_count = profile.tree().nodes().len();
    write_new_file(output, &serde_json::to_vec_pretty(&profile)?)?;
    println!(
        "Imported {node_count} raw macro-tree nodes to {}.",
        output.display()
    );
    Ok(())
}

fn export_tree(input: &Path, output: &Path) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let node_count = profile.tree().nodes().len();
    write_new_file(output, &profile.to_dct_bytes())?;
    println!(
        "Exported {node_count} raw macro-tree nodes to {}.",
        output.display()
    );
    Ok(())
}

fn rename_tree_node(input: &Path, output: &Path, node_index: usize, name: &str) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let tree = profile
        .tree()
        .rename_node(node_index, name)
        .with_context(|| format!("rename macro-tree node {node_index}"))?;
    let profile = AulaOemMacroTreeProfile::new(tree);
    write_tree_profile_or_dct(output, &profile)?;
    println!("Renamed node {node_index} and wrote {}.", output.display());
    Ok(())
}

fn create_tree_folder(input: &Path, output: &Path, parent_id: u32, name: &str) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let tree = profile.tree();
    let node_id = tree.next_available_node_id(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.subsec_nanos()),
    );
    let tree = tree
        .create_folder(parent_id, node_id, name)
        .with_context(|| format!("create macro-tree folder under 0x{parent_id:08X}"))?;
    let profile = AulaOemMacroTreeProfile::new(tree);
    write_tree_profile_or_dct(output, &profile)?;
    println!(
        "Created folder {name:?} (id 0x{node_id:08X}) and wrote {}.",
        output.display()
    );
    Ok(())
}

fn create_tree_macro(input: &Path, output: &Path, parent_id: u32, name: &str) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let tree = profile.tree();
    let node_id = tree.next_available_node_id(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.subsec_nanos()),
    );
    let tree = tree
        .create_macro(parent_id, node_id, name)
        .with_context(|| format!("create macro-tree macro under 0x{parent_id:08X}"))?;
    let profile = AulaOemMacroTreeProfile::new(tree);
    write_tree_profile_or_dct(output, &profile)?;
    println!(
        "Created empty macro {name:?} (id 0x{node_id:08X}) and wrote {}.",
        output.display()
    );
    Ok(())
}

fn delete_tree_node(input: &Path, output: &Path, node_index: usize) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let node_name = profile
        .tree()
        .nodes()
        .get(node_index)
        .with_context(|| format!("node index {node_index} is out of range"))?
        .name()
        .unwrap_or_else(|| format!("node {}", node_index + 1));
    let tree = profile
        .tree()
        .remove_node(node_index)
        .with_context(|| format!("delete macro-tree node {node_index}"))?;
    write_tree_profile_or_dct(output, &AulaOemMacroTreeProfile::new(tree))?;
    println!(
        "Deleted leaf node {node_name:?} and wrote {}.",
        output.display()
    );
    Ok(())
}

fn parse_node_id(value: &str) -> std::result::Result<u32, String> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    u32::from_str_radix(digits, 16).map_err(|error| format!("invalid hexadecimal node id: {error}"))
}

fn inspect_tree(input: &Path) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let nodes = profile.tree().nodes();
    println!("{} nodes", nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let event_count = node.macro_file().map_or_else(
            || "no-macro".to_owned(),
            |file| {
                format!(
                    "{}-visible/{}-raw-events",
                    file.oem_editor_event_count(),
                    file.records().len()
                )
            },
        );
        println!(
            "{index:04} id=0x{:08X} parent=0x{:08X} name={:?} kind=0x{:08X} {event_count}",
            node.node_id(),
            node.parent_id(),
            node.name(),
            node.kind_code(),
        );
    }
    Ok(())
}

fn extract_tree_macro(input: &Path, node_index: usize, output: &Path) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let node = profile
        .tree()
        .nodes()
        .get(node_index)
        .with_context(|| format!("node index {node_index} is out of range"))?;
    let macro_file = node
        .macro_file()
        .with_context(|| format!("node {node_index} does not contain a macro block"))?;
    write_new_file(output, &macro_file.to_oem_bytes())?;
    println!("Extracted node {node_index} to {}.", output.display());
    Ok(())
}

fn encode_tree_transfer_block(input: &Path, node_index: usize, output: &Path) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let node = profile
        .tree()
        .nodes()
        .get(node_index)
        .with_context(|| format!("node index {node_index} is out of range"))?;
    let macro_file = node
        .macro_file()
        .with_context(|| format!("node {node_index} does not contain a macro block"))?;
    let block = aula_oem_macro_transfer_block(macro_file)
        .context("macro cannot be represented by the recovered OEM transfer encoder")?;
    write_new_file(output, &block)?;
    println!(
        "Wrote {}-byte per-macro OEM transfer block for node {node_index} to {}. This is a static local encoding, not a HID report or device write.",
        block.len(),
        output.display()
    );
    Ok(())
}

fn encode_tree_transfer_table(input: &Path, node_indices: &[usize], output: &Path) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let macros = node_indices
        .iter()
        .map(|node_index| {
            let node = profile
                .tree()
                .nodes()
                .get(*node_index)
                .with_context(|| format!("node index {node_index} is out of range"))?;
            node.macro_file()
                .with_context(|| format!("node {node_index} does not contain a macro block"))
        })
        .collect::<Result<Vec<_>>>()?;
    let table = aula_oem_macro_transfer_table(&macros)
        .context("macro nodes cannot be represented by the recovered OEM transfer table")?;
    write_new_file(output, table.bytes())?;
    println!(
        "Wrote {}-byte local OEM macro transfer table with {} unique macro nodes to {}. Node order must match first key-reference order; this is not a HID report or device write.",
        table.bytes().len(),
        table.entries().len(),
        output.display()
    );
    for entry in table.entries() {
        println!(
            "node=0x{:08X} index={} offset={} length={}",
            entry.node_id(),
            entry.index(),
            entry.offset(),
            entry.length()
        );
    }
    Ok(())
}

fn edit_tree_event(
    input: &Path,
    output: &Path,
    node_index: usize,
    event_index: usize,
    record: Option<&str>,
    insert: bool,
) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let replacement = record
        .map(AulaOemMacroEvent::parse_record_hex)
        .transpose()?;
    let edited = match (replacement, insert) {
        (Some(record), true) => profile.tree().insert_event(node_index, event_index, record),
        (Some(record), false) => profile
            .tree()
            .replace_event(node_index, event_index, record),
        (None, false) => profile.tree().remove_event(node_index, event_index),
        (None, true) => bail!("tree insertion requires a raw event record"),
    };
    let tree = edited
        .with_context(|| format!("edit event {event_index} in macro-tree node {node_index}"))?;
    write_tree_profile_or_dct(output, &AulaOemMacroTreeProfile::new(tree))?;
    println!(
        "Updated event {event_index} in node {node_index} and wrote {}. Event semantics remain unverified.",
        output.display()
    );
    Ok(())
}

fn swap_tree_events(
    input: &Path,
    output: &Path,
    node_index: usize,
    first_event_index: usize,
    second_event_index: usize,
) -> Result<()> {
    let profile = read_tree_profile(input)?;
    let tree = profile
        .tree()
        .swap_events(node_index, first_event_index, second_event_index)
        .with_context(|| {
            format!(
                "swap events {first_event_index} and {second_event_index} in macro-tree node {node_index}"
            )
        })?;
    write_tree_profile_or_dct(output, &AulaOemMacroTreeProfile::new(tree))?;
    println!(
        "Swapped raw events {first_event_index} and {second_event_index} in node {node_index} and wrote {}.",
        output.display()
    );
    Ok(())
}

fn read_tree_profile(path: &Path) -> Result<AulaOemMacroTreeProfile> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read AULA macro tree {}", path.display()))?;
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dct"))
    {
        AulaOemMacroTreeProfile::from_dct_bytes(&bytes)
            .context("invalid or unsupported AULA local macro tree")
    } else {
        serde_json::from_slice::<AulaOemMacroTreeProfile>(&bytes)
            .context("invalid or unsupported AULA macro-tree profile")
    }
}

fn write_tree_profile_or_dct(path: &Path, profile: &AulaOemMacroTreeProfile) -> Result<()> {
    let bytes = if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dct"))
    {
        profile.to_dct_bytes()
    } else {
        serde_json::to_vec_pretty(profile).context("encode AULA macro-tree profile")?
    };
    write_new_file(path, &bytes)
}

fn replace_event(input: &Path, output: &Path, index: usize, record: &str) -> Result<()> {
    let profile = read_profile(input)?;
    let replacement = AulaOemMacroEvent::parse_record_hex(record)?;
    let event_count = profile.macro_file().records().len();
    let edited = profile
        .macro_file()
        .replace_record(index, replacement)
        .with_context(|| format!("event index {index} is out of range for {event_count} events"))?;
    write_edited_macro(output, &edited)?;
    println!(
        "Replaced raw event {index} in {event_count} events and wrote {}. Event semantics remain unverified.",
        output.display()
    );
    Ok(())
}

fn insert_event(input: &Path, output: &Path, index: usize, record: &str) -> Result<()> {
    let profile = read_profile(input)?;
    let replacement = AulaOemMacroEvent::parse_record_hex(record)?;
    let original_count = profile.macro_file().records().len();
    let edited = profile
        .macro_file()
        .insert_record(index, replacement)
        .with_context(|| {
            format!("insertion index {index} is invalid or exceeds the macro record limit")
        })?;
    write_edited_macro(output, &edited)?;
    println!(
        "Inserted one raw event at index {index} ({original_count} to {} events) and wrote {}. Event semantics remain unverified.",
        edited.records().len(),
        output.display()
    );
    Ok(())
}

fn remove_event(input: &Path, output: &Path, index: usize) -> Result<()> {
    let profile = read_profile(input)?;
    let original_count = profile.macro_file().records().len();
    let edited = profile.macro_file().remove_record(index).with_context(|| {
        format!("event index {index} is out of range for {original_count} events")
    })?;
    write_edited_macro(output, &edited)?;
    println!(
        "Removed raw event {index} ({original_count} to {} events) and wrote {}. Event semantics remain unverified.",
        edited.records().len(),
        output.display()
    );
    Ok(())
}

fn swap_events(input: &Path, output: &Path, first_index: usize, second_index: usize) -> Result<()> {
    let profile = read_profile(input)?;
    let event_count = profile.macro_file().records().len();
    let edited = profile
        .macro_file()
        .swap_records(first_index, second_index)
        .with_context(|| {
            format!(
                "event indexes {first_index} and {second_index} must both be in range for {event_count} events"
            )
        })?;
    write_edited_macro(output, &edited)?;
    println!(
        "Swapped raw events {first_index} and {second_index} in {event_count} events and wrote {}.",
        output.display()
    );
    Ok(())
}

fn write_edited_macro(path: &Path, macro_file: &AulaOemMacroFile) -> Result<()> {
    let edited = AulaOemMacroProfile::new(macro_file.clone());
    let bytes = if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mcr"))
    {
        edited.to_oem_bytes()
    } else {
        serde_json::to_vec_pretty(&edited)?
    };
    write_new_file(path, &bytes)
}

fn encode_macro_file_transfer_block(input: &Path, output: &Path) -> Result<()> {
    let profile = read_profile(input)?;
    let block = aula_oem_macro_transfer_block(profile.macro_file())
        .context("macro cannot be represented by the recovered OEM transfer encoder")?;
    write_new_file(output, &block)?;
    println!(
        "Wrote {}-byte per-macro OEM transfer block to {}. This is a static local encoding, not a HID report or device write.",
        block.len(),
        output.display()
    );
    Ok(())
}

fn read_profile(path: &Path) -> Result<AulaOemMacroProfile> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read AULA macro file {}", path.display()))?;
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mcr"))
    {
        AulaOemMacroProfile::from_oem_bytes(&bytes)
            .context("invalid or unsupported AULA OEM macro file")
    } else {
        serde_json::from_slice::<AulaOemMacroProfile>(&bytes)
            .context("invalid or unsupported AULA macro profile")
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "create output {} (existing files are preserved)",
                path.display()
            )
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error).with_context(|| format!("write output {}", path.display()));
    }
    Ok(())
}
