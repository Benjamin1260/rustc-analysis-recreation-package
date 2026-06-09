use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, fmt::Display, path::PathBuf};

use crate::utils::db::Repo;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrateOrigin {
    Workspace(bool), // is crate compile-target ?
    Dependency,
    Sysroot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateMetadata {
    pub name: String,
    pub version: String,
    pub origin: CrateOrigin,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateMetadataIndex {
    alias_to_idx: HashMap<String, Vec<usize>>,
    crate_metadata_ls: Vec<CrateMetadata>,
    pub local_crate: CrateMetadata,
}

impl CrateOrigin {
    pub fn is_crate_compile_target(&self) -> bool {
        match self {
            Self::Workspace(b) => *b,
            _ => false,
        }
    }

    // TODO: SHOULD remove this and in-lieu update the database scheme
    #[deprecated]
    pub fn is_internal(&self) -> bool {
        match self {
            Self::Workspace(_) => true,
            _ => false,
        }
    }
}

impl CrateMetadata {
    pub fn from_package(package: &Package, workspace_members: &HashSet<PackageId>, is_root_package: bool) -> Self {
        let origin: CrateOrigin = if workspace_members.contains(&package.id) {
            CrateOrigin::Workspace(is_root_package)
        } else {
            CrateOrigin::Dependency
        };

        Self {
            name: package.name.to_string(),
            version: package.version.to_string(),
            origin: origin,
            path: package.manifest_path.parent().unwrap().into(),
        }
    }
}

impl Display for CrateMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl CrateMetadataIndex {
    pub fn from_path(cwd: PathBuf, repo: &Repo) -> Self {
        let mut cmd = MetadataCommand::new();
        cmd.current_dir(cwd);

        match &repo.cargo_args {
            Some(args) => {cmd.other_options(shlex::split(args).expect("invalid cargo args"));},
            None => {},
        }

        CrateMetadataIndex::new(cmd.exec().unwrap())
    }

    pub fn new(metadata: Metadata) -> Self {
        let workspace_members: HashSet<PackageId> = metadata.workspace_members.iter().cloned().collect();
        let local_package = metadata.root_package().unwrap().clone();
        let mut alias_to_idx: HashMap<String, Vec<usize>> = HashMap::with_capacity(metadata.packages.len());
        let mut crate_metadata_ls: Vec<CrateMetadata> = Vec::with_capacity(metadata.packages.len());

        for package in metadata.packages {
            let crate_metadata = CrateMetadata::from_package(&package, &workspace_members, package.eq(&local_package));
            let idx = crate_metadata_ls.len();
            crate_metadata_ls.push(crate_metadata);

            let package_alias_ls: Vec<String> = package.targets.iter()
                //.map(|target| target.name.replace("-", "_")) // might be different in rustc and cargo, we default to _
                .map(|target| target.name.clone()) // changing name should not be required since using target name
                .collect();

            for alias in package_alias_ls {
                alias_to_idx.entry(alias)
                    .or_default()
                    .push(idx);
            }
        }

        Self{
            alias_to_idx: alias_to_idx,
            crate_metadata_ls: crate_metadata_ls,
            local_crate: CrateMetadata::from_package(&local_package, &workspace_members, true),
        }
    }

    fn get_crate_metadata_ls_from_alias(&self, alias: &String) -> Vec<&CrateMetadata> {
        // replace from - -> _ should not be needed since using target name verbatim
        let idx_ls = self.alias_to_idx.get(alias).expect("alias unknown").clone(); // TODO: return closest matches here // should not be necessary anymore since using targets
        self.crate_ls_from_idx_ls(idx_ls)
    }

    pub fn find_crates_with_alias(&mut self, crate_name: String, rustc_path: PathBuf) -> Vec<&CrateMetadata> {
        // let crate_name = crate_name.replace("-", "_"); // might be different in rustc and cargo, we default to _

        // special case: std crates are not in added by cargo_metadata
        // we check if they are present and if they are not, we add them
        if Self::is_sysroot(&rustc_path) {
            println!("Attention! Sysroot detected: {:?}", crate_name.clone());
            let idx_ls = self.alias_to_idx.get(&crate_name);


            if idx_ls.is_none() {
                return vec![self.add_sysroot_crate(crate_name, rustc_path.clone())]
            }
            
            let idx_ls: Vec<usize> = idx_ls.unwrap().into_iter()
                .filter(|idx| self.crate_metadata_ls[**idx].path.eq(&rustc_path))
                .map(|idx| idx.clone())
                .collect();
            
            match idx_ls.len() {
                0 => return vec![self.add_sysroot_crate(crate_name, rustc_path.clone())],
                1 => return self.crate_ls_from_idx_ls(idx_ls),
                _ => panic!("ERROR: sysroot crates should be resolved here!\nExpected single crate, got: {:#?}", self.crate_ls_from_idx_ls(idx_ls)),
            }
        }

        // regular crate (either CrateOrigin::Workspace or CrateOrigin::Dependency)
        return self.get_crate_metadata_ls_from_alias(&crate_name);
    }

    fn is_sysroot(rustc_path: &PathBuf) -> bool {
        rustc_path
            .components()
            .any(|c| c.as_os_str() == "rustlib")
    }

    fn add_sysroot_crate(&mut self, crate_name: String, rustc_path: PathBuf) -> &CrateMetadata {
        let idx = self.crate_metadata_ls.len();

        self.crate_metadata_ls.push(CrateMetadata { 
            name: crate_name.clone(), 
            version: "?".to_owned(), 
            origin: CrateOrigin::Sysroot, 
            path: rustc_path,
        });

        self.alias_to_idx
            .entry(crate_name.clone())
            .or_default()
            .push(idx);

        &self.crate_metadata_ls[idx]
    }

    fn crate_ls_from_idx_ls(&self, idx_ls: Vec<usize>) -> Vec<&CrateMetadata> {
        idx_ls.into_iter().map(|idx| &self.crate_metadata_ls[idx]).collect()
    }

    // fn find_crate_match_path(candidates: &Vec<CrateMetadata>, rustc_path: PathBuf) -> Option<CrateMetadata> {
    //     let rustc_path_components = rustc_path.components();
    //     let filtered_candidates = candidates.iter()
    //         .filter(|c| {
    //             let crate_workspace_dir = c.path.file_name().unwrap();

    //         });

    //     match filtered_candidates.len() {
    //         1 => Some(filtered_candidates[0].clone()),
    //         _ => panic!("could not disambiguate crates with identical name using path!\n\nTarget: {}\nOptions: {:#?}", rustc_path.display(), candidates),
    //     }
    // }
}
