use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, path::PathBuf};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateMetadata {
    pub name: String,
    pub version: String,
    pub internal: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoMetadataIndex {
    pub by_crate_name: HashMap<String, Vec<CrateMetadata>>,
}

impl CrateMetadata {
    pub fn from_package(package: Package, workspace_members: &HashSet<PackageId>) -> Self {
        Self {
            name: package.name.to_string(),
            version: package.version.to_string(),
            internal: workspace_members.contains(&package.id),
            path: package.manifest_path.parent().unwrap().into(),
        }
    }
}

impl CargoMetadataIndex {
    pub fn new(metadata: Metadata) -> Self {
        let workspace_members: HashSet<PackageId> = metadata.workspace_members.iter().cloned().collect();
        let mut by_crate_name: HashMap<String, Vec<CrateMetadata>> = HashMap::with_capacity(metadata.packages.len());

        for package in metadata.packages {
            let package_name = package.name.to_string().replace("-", "_"); // might be different in rustc and cargo, we default to _
            let crate_metadata = CrateMetadata::from_package(package, &workspace_members);

            match by_crate_name.get_mut(&package_name) {
                Some(vec) => {vec.push(crate_metadata)}
                None => {by_crate_name.insert(package_name, vec![crate_metadata]);}
            }
        }

        Self{
            by_crate_name: by_crate_name,
        }
    }

    pub fn find_crate(&self, crate_name: String, rustc_path: PathBuf) -> Option<CrateMetadata> {
        let crate_name = crate_name.replace("-", "_"); // might be different in rustc and cargo, we default to _

        // special case: any std crates are not in CargoMetadataIndex
        let is_sysroot = rustc_path
            .components()
            .any(|c| c.as_os_str() == "rustlib");

        if is_sysroot {
            println!("Attention! STD detected: {:?}", crate_name);
            // TODO: MUST remove this default return statement, THIS IS WRONG, should be fetched from somewhere/be universal/shared
            // should use the CargoMetadataIndex.by_crate_name
            // side-note, what is this function used for? how do we use these crates/results
            return Some(CrateMetadata { name: crate_name, version: "?".to_owned(), internal: false, path: rustc_path });
        }

        let candidates = self.by_crate_name.get(&crate_name)?;

        match candidates.len() {
            0 => None,
            1 => Some(candidates[0].clone()),
            _ => panic!("TODO: SHOULD implement this")
        }
    }
}

impl From<PathBuf> for CargoMetadataIndex {
    fn from(cwd: PathBuf) -> Self {
        let metadata = MetadataCommand::new()
            .current_dir(cwd)
            .exec().unwrap();

        CargoMetadataIndex::new(metadata)
    }
}