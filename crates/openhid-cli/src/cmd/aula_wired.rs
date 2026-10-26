//! Backup, inspect and restore the verified F87 Pro wired config block.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use clap::Subcommand;
use openhid_hid::aula::WiredConfigBlock;

/// Wired AULA configuration block backup and restore commands.
#[derive(Debug, Subcommand)]
pub enum AulaWiredCmd {
    /// Read the complete 128-byte config block into a new binary backup file.
    ReadConfig {
        /// Exact identity printed by `openhid diag aula --wired-model-probe`.
        #[arg(long)]
        identity: String,
        /// New output file; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Experimentally read an opaque 4096-byte macro-table candidate; performs no macro writes.
    ReadMacros {
        /// Exact identity printed by `openhid diag aula --wired-model-probe`.
        #[arg(long)]
        identity: String,
        /// New output file; existing files are preserved.
        #[arg(long)]
        output: PathBuf,
    },
    /// Persistently replace the full config block from a validated backup file.
    WriteConfig {
        /// Exact identity printed by `openhid diag aula --wired-model-probe`.
        #[arg(long)]
        identity: String,
        /// Exact 128-byte config block previously exported by `read-config`.
        #[arg(long)]
        input: PathBuf,
        /// Required confirmation that the persistent write may proceed.
        #[arg(long)]
        apply: bool,
    },
    /// Refuse wired macro-table writes until the OEM protocol is verified.
    WriteMacros {
        /// Exact identity printed by `openhid diag aula --wired-model-probe`.
        #[arg(long)]
        identity: String,
        /// Exact 4096-byte table prepared from a complete wired macro-table read.
        #[arg(long)]
        input: PathBuf,
        /// New path for a complete device macro-table backup; existing files are preserved.
        #[arg(long)]
        backup: PathBuf,
        /// Required confirmation that the experimental persistent write may proceed.
        #[arg(long)]
        apply: bool,
    },
}

impl AulaWiredCmd {
    /// Run one wired configuration operation.
    pub async fn run(self) -> Result<()> {
        match self {
            Self::ReadConfig { identity, output } => {
                let block = openhid_hid::read_aula_wired_config(Some(&identity)).await?;
                write_new_file(&output, block.as_bytes())?;
                println!(
                    "Saved the validated F87 Pro wired configuration block to {}.",
                    output.display()
                );
            }
            Self::ReadMacros { identity, output } => {
                let table = openhid_hid::read_aula_wired_macro_table(Some(&identity)).await?;
                write_new_file(&output, table.as_bytes())?;
                println!(
                    "Saved the experimental wired macro-table candidate to {}. The format and completeness are unverified; no macro data was written.",
                    output.display()
                );
            }
            Self::WriteConfig {
                identity,
                input,
                apply,
            } => {
                if !apply {
                    bail!("wired config writes persist immediately; pass --apply to continue");
                }
                let bytes = std::fs::read(&input)
                    .with_context(|| format!("read wired config block {}", input.display()))?;
                let block = WiredConfigBlock::from_bytes(&bytes)
                    .context("validate wired AULA config block")?;
                openhid_hid::write_aula_wired_config(Some(&identity), &block).await?;
                println!(
                    "The wired F87 Pro config block was written and verified from device readback."
                );
            }
            Self::WriteMacros { .. } => {
                bail!(
                    "AULA wired macro writes are disabled until the macro read protocol produces a stable, complete backup"
                );
            }
        }
        Ok(())
    }
}

fn write_new_file(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| {
            format!(
                "create backup {} (existing files are preserved)",
                path.display()
            )
        })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
