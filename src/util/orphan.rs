use super::find_links;
use git2::{Object, Repository};
use std::{collections::HashSet, path::PathBuf};

pub fn get_orphans(repo: &Repository, commit: Object) -> HashSet<PathBuf> {
    let commit = commit.into_commit().unwrap();
    let tree = commit.tree().unwrap();
    let mut pages = HashSet::new();
    let mut links = HashSet::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        let Ok(obj) = entry.to_object(repo) else {
            return 0;
        };
        let Ok(blob) = obj.into_blob() else { return 0 };
        if 0o120000 == entry.filemode() {
            return 0;
        }
        let name = entry.name().unwrap();
        let fname: PathBuf = format!("/{dir}{}", name).into();
        if let Some(true) = fname.extension().map(|e| e == "org") {
            find_links(&fname, blob, &mut links);
            // index is always linked by header nav
            if name != "index.org" {
                pages.insert(fname);
            }
        }
        0
    })
    .unwrap();

    pages.difference(&links).cloned().collect()
}
