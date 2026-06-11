extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_hir_id;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_public;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_type_ir;

use std::path::PathBuf;

use rustc_analysis::utils::{analysis_results::AnalysisResults, crate_metadata_index::{CrateMetadata, CrateMetadataIndex}, db::*, directed_graph::DirectedGraph};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::{CRATE_DEF_INDEX, CrateNum, DefId, LOCAL_CRATE, LocalDefId};
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;
use rustc_span::{FileName, RealFileName, RemapPathScopeComponents};

#[derive(Debug)]
pub struct AnalysisCallback {
    pub repo_id: u32,
    pub crate_metadata_index: CrateMetadataIndex,

    pub data: AnalysisResults,
}


impl AnalysisCallback {
    fn fill(&mut self, graph: DirectedGraph<DefId>, tcx: &TyCtxt) {
        self.data.def_ids.reserve(graph.outgoing.len());

        for (def_id, outgoing) in graph.outgoing.into_iter() {
            // def_ids
            self.data.def_ids.push(new_def_id_row(&tcx, def_id, self.repo_id));

            // dependencies
            self.data.dependencies.extend(outgoing.into_iter()
                .map(|def_id_to| new_dependency(&tcx, def_id, def_id_to))
            );
        }

        // tcx.crates() does not include the compile target itself
        // so we get the LOCAL_CRATE CrateNum and add it manually

        /*
        let local_crate_metadata = self.crate_metadata_index.local_crate.clone();
        let local_crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
        if !local_crate_metadata.name.eq(&local_crate_name) {
            eprintln!("WARNING: local_crate_metadata.name does not match rustc::LOCAL_CRATE [{}!={}]", local_crate_metadata, local_crate_name);
            // TODO: SHOULD check if this warning gets printed on `build_script_build` in `vaultwarden`
        }

        self.data.crates.push(Crate {
            stable_crate_id: tcx.stable_crate_id(LOCAL_CRATE).as_u64(),
            name: local_crate_name,
            version: local_crate_metadata.version,
            internal: local_crate_metadata.origin.is_internal(), // TODO: SHOULD remove internal
            path_url: local_crate_metadata.path.into_string().unwrap(),
            merged_crate_id: None,
        });
        */
        let local_crate_path = PathBuf::default(); // rustc_path is only used if crate is CrateOrigin::Sysroot, since it is compile target, it is CrateOrigin::Workspace(True) so should be unused
        let local_crate = new_crate(&tcx, &mut self.crate_metadata_index, self.repo_id, LOCAL_CRATE, local_crate_path);
        self.data.crates.push(local_crate);

        // add remaining crates (excluding compile target):
        self.data.crates.extend(tcx.crates(()).iter()
            .map(|crate_num| new_crate(
                &tcx,
                &mut self.crate_metadata_index,
                self.repo_id,
                *crate_num,
                get_path_from_crate_num(&tcx, *crate_num),
            ))
        );

        // TODO: SHOULD init def_id_kinds 
        // should be setup in the front-end upon DB creation
        // should be read from the database file
    }
}

fn new_def_id_row(tcx: &TyCtxt, def_id: DefId, src_repo: u32) -> DefIdRow {
    let hash = tcx.def_path_hash(def_id);

    DefIdRow {
        stable_crate_id: hash.stable_crate_id().as_u64(),
        local_hash: hash.local_hash().as_u64(),
        src_repo: src_repo,
        def_path_str: tcx.def_path_str(def_id),
        kind: 0, // TODO: SHOULD fix this
        nonsafe: false, // TODO: MUST implement this
    }
}

fn new_dependency(tcx: &TyCtxt, def_id_from: DefId, def_id_to: DefId) -> Dependency {
    let from_hash = tcx.def_path_hash(def_id_from);
    let to_hash = tcx.def_path_hash(def_id_to);

    Dependency { 
        from_stable_crate_id: from_hash.stable_crate_id().as_u64(),
        from_local_hash: from_hash.local_hash().as_u64(),
        to_stable_crate_id: to_hash.stable_crate_id().as_u64(),
        to_local_hash: to_hash.local_hash().as_u64(),
    }
}

// we iterate over the crates available to rustc through CrateNum representation
// we must then match this rustc crate to the cargo_metadata crate
// both of these use target aliases, many crates might have a target with identical name
// so if a collision occurs, we have to resolve it using their paths
fn new_crate(tcx: &TyCtxt, cargo_metadata_index: &mut CrateMetadataIndex, _repo_id: u32, crate_num: CrateNum, rustc_path: PathBuf) -> Crate {
    let crate_name = tcx.crate_name(crate_num).to_string();

    let crate_metadata_matches = cargo_metadata_index.find_crates_with_alias(crate_name.clone(), rustc_path.clone());
    let crate_metadata: CrateMetadata = match crate_metadata_matches.len() {
        0 => panic!("unreachable"),
        1 => crate_metadata_matches[0].clone(), // TODO: SHOULD why the f do i gotta clone this if its dropped afterwards
        _ => { 
            // eprintln!("resolving multiple crates returned matching target alias: {}", crate_name);   
            resolve_duplicate_crate_match(&tcx, crate_metadata_matches, crate_num) // resolve using path
        }
    };

    Crate {
        stable_crate_id: tcx.stable_crate_id(crate_num).as_u64(),
        name: crate_name,
        version: crate_metadata.version,
        internal: crate_metadata.origin.is_internal(), // TODO: SHOULD remove internal
        path_url: crate_metadata.path.into_string().unwrap(),
        merged_crate_id: None,
    }
}

fn get_path_from_crate_num(tcx: &TyCtxt, crate_num: CrateNum) -> PathBuf {
    let source = tcx.used_crate_source(crate_num); // NOTE: not supported on LOCAL_CRATE_NUM
    source.rmeta
        .as_ref()
        .or(source.rlib.as_ref())
        .or(source.dylib.as_ref())
        .unwrap()
        .clone()

    // let crate_root = DefId {
    //     krate: crate_num,
    //     index: CRATE_DEF_INDEX,
    // };

    // let source_file = tcx
    //     .sess
    //     .source_map()
    //     .span_to_filename(tcx.def_span(crate_root));

    // let FileName::Real(real_filename) = source_file else {
    //     panic!("FileName was not real!\n{:#?}", source_file);
    // };

    // let path = real_filename.path(RemapPathScopeComponents::DIAGNOSTICS);
    // path.to_path_buf()

    // // FIXME: SHOULD, this is dirty...
    // loop {
    //     let final_part = path.file_name().unwrap();
    //     if final_part.to_str().unwrap().contains("-") {
    //         return PathBuf::from(final_part);
    //     } else {
    //         path = match path.parent() {
    //             Some(path) => path,
    //             None => {panic!("Reached end of path without finding part containing '-'!\n{:#?}", real_filename);},
    //         };
    //     }
    // }

    // let Some(local_path) = real_filename.clone().into_local_path() else {
    //     panic!("FileName did not have local_path!\n{:#?}", real_filename);
    // };

    // local_path
}

// in case multiple target crates have a target with the same name, 
// this method resolves the matches
// FIXME: build_script_build currently returns a long list of crates which we cannot distinguish

// example:
/*
Some(
        InnerRealFileName {
            name: "cli/lib/build.rs",
            working_directory: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5",
            embeddable_name: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5/cli/lib/build.rs",
        },
    ),
    maybe_remapped: InnerRealFileName {
        name: "cli/lib/build.rs",
        working_directory: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5",
        embeddable_name: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5/cli/lib/build.rs",
    },
    scopes: RemapPathScopeComponents(
        0x0,
    ),
}
    
should match

    CrateMetadata {
        name: "deno_lib",
        version: "0.68.0",
        origin: Workspace(
            false,
        ),
        path: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5/cli/lib",
    },
    
and not

    CrateMetadata {
        name: "deno",
        version: "2.8.2",
        origin: Workspace(
            false,
        ),
        path: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5/cli",
    },
    CrateMeta
*/

// FIXME: 
// print the crate name we are looking for and the path we use to look for it
// then print closest matches

// I don't think CrateOrigin::Sysroot can call this

fn resolve_duplicate_crate_match(tcx: &TyCtxt, crate_metadata_matches: Vec<&CrateMetadata>, crate_num: CrateNum) -> CrateMetadata {
    // TODO: COULD assert crate is not sysroot
    // TODO: COULD print diagnostics

    // get FilePath to rustc crate
    let real_file_name: RealFileName = get_real_file_name_for_crate_num(&tcx, crate_num);
    let name = real_file_name.local_path().unwrap();
    let (working_directory, _embeddable_name) = real_file_name.embeddable_name(RemapPathScopeComponents::COVERAGE); // I dont understand which remappathscope to use

    // embeddable_name: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5/cli/lib/build.rs"
    // options[_].path: "/home/user/Coding/school/CSE3000/rec_pack/repositories/5/cli/lib",
    let mut name_traverser = name.parent();
    loop {
        match name_traverser {
            None => { panic!("reached end of workspace without resolving!"); }, // reached end of workspace without finding match
            Some(sub_dir) => {
                let target_path = working_directory.join(sub_dir);
                let target = target_path.to_str().unwrap();

                for cmd in &crate_metadata_matches {
                    if cmd.path.eq(target) {
                        return (*cmd).clone();
                    }
                }

                name_traverser = sub_dir.parent();
            }
        }
    }
    
/*
    // Option2: CrateOrigin::Dependency
    // embeddable_name: "/home/user/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/hashbrown-0.14.5/src/<FILE>"
    /* Option:
        CrateMetadata {
            name: "hashbrown",
            version: "0.14.5",
            origin: Dependency,
            path: "/home/user/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/hashbrown-0.14.5",
        },
     */

    // we need to get section with "<CRATE_NAME>-<VERSION>"
    let embeddable_name_str = embeddable_name.to_str().unwrap();
    let matches: Vec<&&CrateMetadata> = crate_metadata_matches.iter()
        .filter(|cmd| {
            embeddable_name_str.contains(&cmd.name) 
            & embeddable_name_str.contains(&cmd.version)
        }).collect();

    match matches.len() {
        1 => { return (*matches[0]).clone(); },
        _ => { 
            panic!("could not resolve duplicate crate match!\ntarget.embeddable_name: {}\noptions: \n{:#?}", 
                embeddable_name.display(), 
                crate_metadata_matches
            ); 
        },
    }
*/
}

fn get_real_file_name_for_crate_num(tcx: &TyCtxt, crate_num: CrateNum) -> RealFileName {
    let crate_root = DefId {
        krate: crate_num,
        index: CRATE_DEF_INDEX,
    };

    let source_file = tcx
        .sess
        .source_map()
        .span_to_filename(tcx.def_span(crate_root));

    let FileName::Real(real_file_name) = source_file else {
        panic!("FileName was not real!\n{:#?}", source_file);
    };

    real_file_name
}

impl Callbacks for AnalysisCallback {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut fn_def_id_graph: DirectedGraph<DefId> = DirectedGraph::default();
        
        for local_def_id in tcx.hir_body_owners() { // for all HIR bodies, get the def_id so we can access them

            // scenarios:
            // async function outside -> should be empty, can be ignored
            // async function desugared body -> use parent def_id for graph
            // async closure -> use own def_id for graph
            // non-async funtion -> use own def_id for graph
            let (def_id, local_def_id_is_async) = match if_is_coroutine_desugared_fn_get_parent_fn(&tcx, local_def_id) {
                Some(parent_def_id) => (parent_def_id, true),
                None => { match get_async_type_of_local_def_id(&tcx, local_def_id) {
                    AsyncType::Const | AsyncType::Static => { continue; }, // skip since this cannot invoke async
                    ty => (local_def_id, ty.is_async()),
                }}, 
            };

            // create graph of function calls made by this
            let fn_call_def_id_ls: Vec<DefId> = get_calls(tcx.optimized_mir(local_def_id)); // DefId of callee functions
            let fn_call_def_id_ls_filtered: Vec<DefId> = fn_call_def_id_ls
                .iter()
                .filter(|d_id| callee_is_async(&tcx, **d_id))
                .cloned()
                .collect();

            if !local_def_id_is_async && fn_call_def_id_ls_filtered.is_empty() {
                // println!("non-async function did not call any async functions, not registering");
            } else {
                fn_def_id_graph.add_outgoing_from_iter(def_id.to_def_id(), fn_call_def_id_ls_filtered);
            }
        }

        self.fill(fn_def_id_graph, &tcx);
        Compilation::Continue
    }
}


fn get_calls(mir_body: &rustc_middle::mir::Body) -> Vec<DefId> {
    let mut fn_calls_def_id: Vec<DefId> = Vec::new();

    for (_, bbd) in mir_body.basic_blocks.iter_enumerated() {
        let fn_call_def_id_opt: Option<DefId> = match &bbd.terminator().kind {
            rustc_middle::mir::TerminatorKind::Call { func, .. } | 
            rustc_middle::mir::TerminatorKind::TailCall { func, .. }
            => { 
                match func {
                    rustc_middle::mir::Operand::Constant(c) => {
                        let ty = c.const_.ty();

                        match ty.kind() {
                            rustc_middle::ty::TyKind::FnDef(def_id, _) => Some(def_id.clone()),
                            _ => None,
                        }
                    },
                    _ => None,
                }
            },
            _ => continue,
        };

        if let Some(fn_call_def_id) = fn_call_def_id_opt {
            fn_calls_def_id.push(fn_call_def_id);
        }
    }

    fn_calls_def_id
}


enum AsyncType {
    AsyncFn,
    AsyncClosure,
    Const,
    Static,
    NotAsync,
}

impl AsyncType {
    pub fn is_async(self) -> bool {
        match self {
            Self::AsyncFn | Self::AsyncClosure => true,
            Self::Const | Self::Static | Self::NotAsync => false,
        }
    }
}

/// if it is not a function (=not owner), returns false
/// if is a function but not defined with `async`, returns false
/// if function and defined with `async`, returns true
fn get_async_type_of_local_def_id(tcx: &TyCtxt, local_def_id: LocalDefId) -> AsyncType {
    match tcx.hir_body_owner_kind(local_def_id) {
        rustc_hir::BodyOwnerKind::Fn => {
            let hir_id = tcx.local_def_id_to_hir_id(local_def_id);
            if tcx.hir_node(hir_id).fn_sig().expect("function should have signature").header.is_async() {
                return AsyncType::AsyncFn;
            } else {
                return AsyncType::NotAsync;
            }
        },
        rustc_hir::BodyOwnerKind::Closure => {
            match tcx.coroutine_kind(local_def_id) {
                Some(rustc_hir::CoroutineKind::Desugared(rustc_hir::CoroutineDesugaring::Async, _)) => AsyncType::AsyncClosure,
                _ => AsyncType::NotAsync
            }
        },
        rustc_hir::BodyOwnerKind::Const {..} => AsyncType::Const,
        rustc_hir::BodyOwnerKind::Static {..} => AsyncType::Static,
        _ => AsyncType::NotAsync
    }
}

/// this function fetches the HIR header for a function and checks if it is async
/// if the `DefId` is not of a function, or not async, returns false 
fn callee_is_async(tcx: &TyCtxt, def_id: DefId) -> bool {
    // apparently, it is impossible to fully recover function headers from external crates
    // thus we distinguish the following two cases:
    // case1(internal crate): we check the function signature for `async`
    // case2(external crate): we check if the return type is implements Future

    match DefId::as_local(def_id) {
        Some(local_def_id) => get_async_type_of_local_def_id(tcx, local_def_id).is_async(),
        None => {
            // println!("Parsing External Function: {:?}", def_id.clone());
            
            // NOTE: block below comes from chatGPT (has to be tweaked/fixed)
            let fn_sig = tcx.fn_sig(def_id).instantiate_identity().skip_binder();
            let output = fn_sig.output();

            if let rustc_middle::ty::TyKind::Alias(alias) = output.kind() {
                if let rustc_type_ir::AliasTy{kind: rustc_type_ir::AliasTyKind::Opaque{ def_id: alias_def_id }, ..} = alias {
                    return matches!(tcx.opaque_ty_origin(*alias_def_id), rustc_hir::OpaqueTyOrigin::AsyncFn{..});
                }
            }

            // println!("External Function was not async!");
            false
        }
    }
}


/// given a coroutine (f1::closure#0), it checks if it is desugared from
/// `async f1() {...}` and returns DefId of `f1`, else returns None
/// 
/// # Example
/// async fn f0() {f1()} async fn f1() implies
/// f0 -> {f1}, f1::closure#0 -> {...}
/// since f1!=f1::closure#0 , this breaks the dependency chain
/// we thus need to merge a desugared closure with it's original function
fn if_is_coroutine_desugared_fn_get_parent_fn(tcx: &TyCtxt, local_def_id: LocalDefId) -> Option<LocalDefId> {
    match tcx.coroutine_kind(local_def_id) {
        Some(rustc_hir::CoroutineKind::Desugared(rustc_hir::CoroutineDesugaring::Async, rustc_hir::CoroutineSource::Fn)) => Some(tcx.local_parent(local_def_id)),
        _ => None,
    }
}