use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about = "RustC-based static analysis tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args, Debug)]
pub struct CommonArgs {
    pub target_dir: PathBuf,

    #[arg(long)]
    pub cargo_args: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    // TODO: COULD remove this
    /// Run analysis on async functions and dependencies and output to stdout
    FnDepTree {
        #[command(flatten)]
        common: CommonArgs,
    },

    #[command(
        about = "Run pipeline stage 1 and 2",
        long_about = "Run pipeline stage 1 and 2, requires number of repositories specified\n\nstage1: create .duckdb file\nstage2: insert repository list into .duckdb file",
    )]
    InitRepoList{
        /// number of repositories to fetch urls for
        repo_count: u32,

        /// path where duckdb output file will be stored
        duckdb_path: PathBuf,
    },
    
    #[command(
        about = "Run pipeline stage 3 and 4",
        long_about = "Run pipeline stage 3 and 4\n\nstage3: fetch repository list from github\n\nstage4: analyze repository list",
    )]
    Analyze {
        /// path to duckdb file with repositories to analyze
        duckdb_path: PathBuf,

        #[arg(default_value = "./repositories")]
        /// path to dir where repositories are/will be stored
        repo_dir_path: PathBuf,

        #[arg(long)]
        /// true means existing folders with repo_id will be reused as such
        enable_reuse: bool,
    },
}

impl Commands {
    pub const FN_DEP_TREE: &'static str = "fn-dep-tree";
    pub const INIT_REPO_LIST: &'static str = "init-repo-list";
    pub const ANALYZE: &'static str = "analyze";

    pub fn as_env_str(&self) -> &'static str {
        match self {
            Commands::FnDepTree {..} => Commands::FN_DEP_TREE,
            Commands::InitRepoList {..} => Commands::INIT_REPO_LIST,
            Commands::Analyze {..} => Commands::ANALYZE,
        }
    }
}