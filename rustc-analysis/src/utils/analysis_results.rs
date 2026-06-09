use std::collections::HashMap;

use crate::utils::db::*;

#[derive(Debug, Default)]
pub struct AnalysisResults {
    pub crates: Vec<Crate>,
    pub def_id_kinds: HashMap<u32, DefIdKind>,
    pub def_ids: Vec<DefIdRow>,
    pub dependencies: Vec<Dependency>,
}