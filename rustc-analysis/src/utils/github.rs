use std::{fs::DirBuilder, path::PathBuf, process::Command};

use futures_util::{StreamExt, TryStreamExt};
use octocrab::{Octocrab, models::Repository};

use crate::utils::db::{DB, Repo};

pub async fn fetch_repo_urls(num: usize) -> Result<Vec<String>, octocrab::Error> {
    let crab = Octocrab::default();

    let repos: Vec<Repository> = crab
        .search()
        .repositories("language:rust")
        .sort("stars")
        .order("desc")
        .send()
        .await?
        .into_stream(&crab)
        .take(num)
        .try_collect()
        .await?;

    let url_list: Vec<String> = repos
        .into_iter()
        .map(|r| r.clone_url.unwrap().into())
        .collect();

    Ok(url_list)
}

pub fn clone_repo(repo: Repo, target_dir: PathBuf, conn: &DB, reuse_cloned: bool) {    
    // NOTE: you cannot clone into non-empty dir
    // if dir exists, we NEVER clone
    // if you want to do this in the future, first empty dir before cloning
    if target_dir.exists() {
        if reuse_cloned {
            return;
        } else {
            panic!("dir {:?} already exists!\nrun with --enable_reuse to reuse cloned files", target_dir)
        }
    } else {
        match DirBuilder::new().recursive(true).create(target_dir.as_path()) {
            Ok(_) => {},
            Err(err) => {panic!("failed to create dir {:?}\n{:?}", target_dir, err)},
        }


        match repo.commit_hash {
            Some(commit_hash) => {
                clone_specific_commit(target_dir, repo.repo_url, commit_hash);
            },
            None => {
                let hash = clone_latest_and_get_hash(target_dir, repo.repo_url);
                conn.update_commit_hash(repo.id, hash);
            },
        }
    }
}

fn clone_specific_commit(dir: PathBuf, clone_url: String, hash: String) {
    Command::new("git")
        .arg("init")
        .arg(&dir)
        .status().unwrap();

    Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["remote", "add", "origin"])
        .arg(&clone_url)
        .output().unwrap();

    Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["fetch", "--depth", "1", "origin"])
        .arg(&hash)
        .output().unwrap();

    Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["checkout", "FETCH_HEAD"])
        .output().unwrap();
}

fn clone_latest_and_get_hash(dir: PathBuf, clone_url: String) -> String {
    // clone into dir
    let output = Command::new("git")
        .args(["clone", "--depth=1"])
        .arg(&clone_url)
        .arg(&dir)
        .output().unwrap();

    if !output.status.success() {
        panic!("failed to clone url{{{:?}}} into {:?}", clone_url, dir.as_os_str());
    }

    let hash = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["rev-parse", "HEAD"])
        .output().unwrap();

    String::from_utf8(hash.stdout).unwrap().trim().to_string()
}