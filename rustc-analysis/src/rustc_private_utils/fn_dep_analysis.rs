extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_hir_id;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_public;
extern crate rustc_span;
extern crate rustc_type_ir;

use rustc_analysis::utils::directed_graph::DirectedGraph;

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_interface::interface::Compiler;
use rustc_middle::ty::TyCtxt;

pub struct CallbacksImpl;

// get a string representation of the file
#[allow(dead_code)]
fn file_name_to_str(file_name: &rustc_span::FileName) -> String {
    match file_name {
        rustc_span::FileName::Real(real_file_name) => { 
            real_file_name.local_path()
                .and_then(|p| p.to_str())
                .unwrap_or_default()
                .to_string()
        },
        _ => "error finding path".to_owned(),
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

// FIXME: re-evaluate everywhere this is used, since it does not consider async closures
// I think this should be split into is_async and is_fn/is_closure
// where calls to async closures/functions should be included
// but special precautions are needed for closures

// add two new functions to check for this
// replace the usages of this functions with new ones
// consider what is being checked and why
// finally, implement said new functions

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


// since we run using `RUSTC_WORKSPACE_WRAPPER`, this is only called on crates inside our workspace
// thus all these bodies returned are defined in our workspace = internal crates
impl Callbacks for CallbacksImpl {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        // let source_map = _compiler.sess.source_map(); // source_map can be used to map a rustc_span to a file location
        let mut fn_def_id_graph: DirectedGraph<DefId> = DirectedGraph::default();
        
        for local_def_id in tcx.hir_body_owners() { // for all HIR bodies, get the def_id so we can access them

            // // get file and function name for this HIR body 
            // let span = tcx.hir_body_owned_by(local_def_id).value.span;
            // let file_name_str = file_name_to_str(&source_map.lookup_char_pos(span.lo()).file.name);
            // let fn_name = tcx.def_path_str(local_def_id);
            // println!("{}::{}", file_name_str, fn_name);
            // println!("{:#?}", tcx.coroutine_kind(local_def_id));

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

            // println!("{:?} -> {:?}", def_ids.clone(), def_ids_filtered.clone());

            if !local_def_id_is_async && fn_call_def_id_ls_filtered.is_empty() {
                // println!("non-async function did not call any async functions, not registering");
            } else {
                fn_def_id_graph.add_outgoing_from_iter(def_id.to_def_id(), fn_call_def_id_ls_filtered);
            }
        }

        println!("{:#?}\n", fn_def_id_graph.clone());
        for fn_def_id in fn_def_id_graph.clone() {
            println!("{}", tcx.def_path_str(fn_def_id));
        }

        Compilation::Continue
    }
}