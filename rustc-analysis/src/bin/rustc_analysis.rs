#![feature(pathbuf_into_string)]

use clap::Parser;
use rustc_analysis::utils::{crate_metadata_index::CrateMetadataIndex, db::{DB, Repo}, github::{clone_repo, fetch_repo_urls}};
use shlex;
use std::{fs::remove_file, io::ErrorKind::NotFound, path::{Path, PathBuf}, process::Command};

use rustc_analysis::utils::cli::{Cli, Commands, CommonArgs};

// TODO: COULD update comments to correctly use `///` and `//`

/// This function handles CLI user input and passes it onto cargo which then invokes the wrapper.
/// Any preprocessing before Cargo is called is handled here
#[tokio::main]
async fn main() {
    // eprintln!("exe = {}", std::env::current_exe().unwrap().display());
    // eprintln!("cwd = {}", std::env::current_dir().unwrap().display());

    let cli = Cli::parse();
    let cmd_type = cli.command.as_env_str();

    match cli.command {
        Commands::FnDepTree { common: CommonArgs {target_dir, cargo_args} } => {
            let mut cmd = build_command(&cargo_args, &target_dir, cmd_type);
            cmd.status()
                .expect("cargo failed");
        },
        Commands::InitRepoList { repo_count, duckdb_path } => {
            // stage1: create and connect duckdb file (if file exists, remove it)
            remove_file_if_present(&duckdb_path);
            let db = DB::open(&duckdb_path.into_string().unwrap()).unwrap();
            db.insert_table_scheme();

            // stage2: fetch and insert list of repo urls
            let urls = fetch_repo_urls(repo_count.try_into().expect("repo_count should be positive"))
                .await
                .unwrap();

            db.insert_repo_urls(urls).unwrap();
        },
        Commands::Analyze { duckdb_path, repo_dir_path, enable_reuse } => {
            let repo_dir_path = repo_dir_path.canonicalize().unwrap(); // for debugging purposes, path from root removes unclarity

            // stage3: fetch data
            let repos: Vec<Repo>;
            {
                let db = DB::open(&duckdb_path.clone().into_string().unwrap()).unwrap();
                repos = db.fetch_repos();
                
                for repo in &repos {
                    let target_dir = repo_dir_path.join(Path::new(&repo.id.to_string()));

                    clone_repo(repo.clone(), target_dir, &db, enable_reuse);
                }

                db.conn.close().unwrap();
            }

            // stage4: run analysis
            for repo in repos {
                if repo.analyzed {
                    println!("Repo {} already analyzed, skipping...", repo.id);
                    continue;
                } 

                let target_dir = repo_dir_path.join(&repo.id.to_string());
                let cmi_path = target_dir.join(CrateMetadataIndex::FILE_NAME);
                println!("DEBUG: writing CMI to: {}", cmi_path.display());

                // stage4_A: build cargo_metadata_index
                let cargo_metadata_index: CrateMetadataIndex = CrateMetadataIndex::from_path(target_dir.clone(), &repo);
                let cmi_serialized = serde_json::to_string(&cargo_metadata_index).unwrap();
                std::fs::write(cmi_path,cmi_serialized).unwrap();

                // stage4_B: invoke wrapper/rustc backend
                cargo_clean_workspace(&target_dir, &repo.cargo_args); // FIXME: SHOULD, errors on any other arg than --manifest-path, should add col to duckdb with manifest path separate from cargo args
                eprintln!("invoking cargo");
                let result = build_command(&repo.cargo_args, &target_dir, cmd_type)
                    // .arg("--jobs=1") TODO: SHOULD fix using multiple threads
                    .env("REPOSITORY_ID", repo.id.to_string())
                    .env("RUSTC_ANALYSIS_OUTPUT", std::fs::canonicalize(duckdb_path.clone()).unwrap().as_os_str())
                    .status().unwrap();

                if result.success() {
                    println!("Analysis successful, writing to db!");
                    let db = DB::open(&duckdb_path.clone().into_string().unwrap()).unwrap();
                    db.set_analyzed_true(repo.id);
                    db.conn.close().unwrap();
                }
            }

            // stage5: duckdb postprocessing
            // TODO: SHOULD implement to already resolve some shared crates like those from rust-lib
        },
    }
}


fn remove_file_if_present(path: &PathBuf) {
    match remove_file(&path) {
        Ok(_) => {},
        Err(err) => {
            match err.kind() {
                NotFound => {}, 
                _ => {panic!("could not remove file")},
            };          
        },
    }
}

fn build_command(cargo_args: &Option<String>, target_dir: &PathBuf, command: &str) -> Command {
    let self_path = std::env::current_exe().expect("failed to get path of this executable");
    let wrapper_path = self_path.with_file_name("rustc_analysis_wrapper");

    let mut cmd = Command::new("cargo");
    cmd.args(["+nightly", "check"]);

    if let Some(args) = cargo_args {
        cmd.args(shlex::split(&args).expect("invalid cargo args"));
    }

    cmd.current_dir(target_dir)
        .env("RUSTC_WORKSPACE_WRAPPER", wrapper_path)
        .env("RUSTC_ANALYSIS_KIND", command);

    return cmd;
}

fn cargo_clean_workspace(target_dir: &PathBuf, cargo_args: &Option<String>) {
    println!("Cleaning {}", target_dir.canonicalize().unwrap().display());

    let mut cmd = Command::new("cargo");
    cmd.current_dir(target_dir)
        .arg("clean")
        .arg("--workspace");

    if let Some(args) = cargo_args {
        cmd.args(shlex::split(&args).expect("invalid cargo args"));
    }

    cmd.status().unwrap();
}
