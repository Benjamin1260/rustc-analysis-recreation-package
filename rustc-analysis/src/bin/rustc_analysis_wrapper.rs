#![feature(rustc_private)]
#![feature(pathbuf_into_string)]

#[path = "../rustc_private_utils/mod.rs"]
mod rustc_private_utils;

extern crate rustc_driver;

use std::{collections::HashMap, fs::OpenOptions, io::Write};

use rustc_private_utils::{analysis_callback::AnalysisCallback, fn_dep_analysis};
use cargo_metadata::MetadataCommand;
use rustc_analysis::utils::{analysis_results::AnalysisResults, cargo_metadata::CargoMetadataIndex, cli::Commands, db::DB};
use rustc_driver::Callbacks;

#[derive(Default)]
struct EmptyCallback;

impl Callbacks for EmptyCallback {
    
}

/// Run using `cargo clean && RUSTC_WORKSPACE_WRAPPER=./../target/debug/rustc-analysis cargo check`
/// This file is invoked by Cargo as a wrapper, it then invokes rustc to start the compilation (checks)
fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    // Remove rustc path inserted by Cargo wrapper protocol
    if args.len() > 1 {
        args.remove(1);
    }

    let analysis_kind = std::env::var("RUSTC_ANALYSIS_KIND")
        .expect("RUSTC_ANALYSIS_KIND should be defined");

    match analysis_kind.as_str() {
        Commands::FN_DEP_TREE => run_fn_dep_analysis(args),
        Commands::ANALYZE => run_analyze(args),
        _ => {println!("invalid RUSTC_ANALYSIS_KIND: {}", analysis_kind);}
    }
}

fn run_fn_dep_analysis(args: Vec<String>) {
    rustc_driver::run_compiler(&args, &mut fn_dep_analysis::CallbacksImpl);
}

fn run_analyze(args: Vec<String>) {
    debug_log("analysis started");
    
    let repo_id = u32::from_str_radix(std::env::var("REPOSITORY_ID").unwrap().as_str(), 10).unwrap();

    let cwd = std::env::current_dir().unwrap();
    let cmi_serialized = std::fs::read_to_string(cwd.with_file_name(".cargo_metadata_index")).unwrap();
    let cargo_metadata_index: CargoMetadataIndex = serde_json::from_str(&cmi_serialized).unwrap();

    let mut analysis_callback = AnalysisCallback {
        repo_id: repo_id,
        cargo_metadata_index: cargo_metadata_index,
        data: AnalysisResults::default(),
    };

    // eprintln!("exe = {}", std::env::current_exe().unwrap().display());
    // eprintln!("cwd = {}", std::env::current_dir().unwrap().display());
    rustc_driver::run_compiler(&args, &mut analysis_callback);
    debug_log("analysis finished");

    let db_path = std::env::var("RUSTC_ANALYSIS_OUTPUT").unwrap();
    {
        let mut db = DB::open(db_path);
        db.save_results(analysis_callback.data);
        db.conn.close().unwrap();
    }
}


fn debug_log(msg: impl AsRef<str>) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/rustc-analysis-wrapper.log")
        .unwrap();

    writeln!(file, "{}", msg.as_ref()).unwrap();
}