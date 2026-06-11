#![feature(rustc_private)]
#![feature(pathbuf_into_string)]

#[path = "../rustc_private_utils/mod.rs"]
mod rustc_private_utils;

extern crate rustc_driver;

use std::{fs::OpenOptions, io::Write, time::Duration};

use rustc_private_utils::{analysis_callback::AnalysisCallback, fn_dep_analysis};
use rustc_analysis::utils::{analysis_results::AnalysisResults, crate_metadata_index::CrateMetadataIndex, cli::Commands, db::DB};
use rustc_driver::Callbacks;

#[allow(dead_code)]
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

    let mut cmi_path = std::env::current_dir().unwrap().join(CrateMetadataIndex::FILE_NAME);
    while !cmi_path.exists() {
        // eprintln!("crate_metadata_index could not be found here: {}\nlooking at parent dir", cmi_path.display());
        cmi_path = cmi_path.parent().expect("parent did not exist!").with_file_name(CrateMetadataIndex::FILE_NAME); // TODO: COULD write better error
    }

    let Ok(cmi_serialized) = std::fs::read_to_string(&cmi_path) else {panic!("could not read from serialized crate_metadata_index: {}", cmi_path.display());};
    let crate_metadata_index: CrateMetadataIndex = serde_json::from_str(&cmi_serialized).unwrap();

    let mut analysis_callback = AnalysisCallback {
        repo_id: repo_id,
        crate_metadata_index: crate_metadata_index,
        data: AnalysisResults::default(),
    };

    // eprintln!("exe = {}", std::env::current_exe().unwrap().display());
    // eprintln!("cwd = {}", std::env::current_dir().unwrap().display());
    rustc_driver::run_compiler(&args, &mut analysis_callback);
    debug_log("analysis finished");

    let db_path = std::env::var("RUSTC_ANALYSIS_OUTPUT").unwrap();
    {
        // TODO: COULD make this async?
        let mut db = DB::open_with_retry(&db_path, Duration::from_secs(600)); // honestly, just waiting usually resolves itself
        db.save_results(analysis_callback.repo_id, analysis_callback.data);
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