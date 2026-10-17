use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Select local tools for your shell")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize .loom in the current project, interactively or from tool requests
    Init {
        #[arg(value_name = "TOOL@VERSION")]
        targets: Vec<String>,
        /// Replace an existing .loom when tool requests are supplied
        #[arg(long, requires = "targets")]
        force: bool,
    },
    /// List local tools and versions
    List { tool: Option<String> },
    /// Find an external executable in the inherited PATH
    Which { command: String },
    /// Emit environment code (applied by the shell integration)
    Use {
        #[arg(value_name = "TOOL@VERSION|FILE")]
        targets: Vec<String>,
    },
    /// Start an interactive shell with the selected local tools
    Shell {
        #[arg(value_name = "TOOL@VERSION|FILE")]
        targets: Vec<String>,
    },
    /// Show the current Loom tool collection and its source
    Current,
    /// Emit code to restore the environment before activation
    Clear,
    /// Emit environment code for explicit evaluation
    Env {
        #[arg(value_name = "TOOL@VERSION|FILE")]
        targets: Vec<String>,
    },
    /// Show the Loom version
    Version,
    #[command(name = "__session-env", hide = true)]
    SessionEnv { payload: PathBuf },
}
