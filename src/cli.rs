use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "tgctl", about = "Declarative Telegram group management")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "telegram.toml")]
    pub config: PathBuf,

    /// Path to state file
    #[arg(short, long, default_value = "tgctl.state.json")]
    pub state: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show what changes would be made
    Plan,
    /// Apply changes to match config
    Apply {
        /// Skip confirmation prompt
        #[arg(long)]
        auto_approve: bool,
    },
    /// Import existing Telegram group state
    Import {
        /// Group username or chat ID
        #[arg(long, allow_hyphen_values = true)]
        chat: String,
        /// Local name for this group in config
        #[arg(long)]
        name: String,
    },
    /// Validate config file syntax
    Validate,
    /// Custom emoji utilities
    Emoji {
        #[command(subcommand)]
        action: EmojiAction,
    },
}

#[derive(Subcommand)]
pub enum EmojiAction {
    /// List custom emoji from an emoji pack
    List {
        /// Pack short name (from the t.me/addstickers/... link)
        #[arg(long)]
        pack: String,
    },
    /// Search custom emoji by emoticon
    Search {
        /// Emoticon to search for (e.g. "📢")
        #[arg(long)]
        query: String,
    },
}
