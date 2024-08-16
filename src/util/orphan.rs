use super::{find_links, map_org};
use git2::{Object, Repository};
use std::{collections::HashSet, path::PathBuf};

pub fn get_orphans(repo: &Repository, commit: Object) -> HashSet<PathBuf> {
    let commit = commit.into_commit().unwrap();
    let mut pages = HashSet::new();
    let mut links = HashSet::new();

    map_org(repo, commit, |blob, fname, name| {
        find_links(&fname, blob, &mut links);
        // index is always linked by header nav
        if name != "index.org" {
            pages.insert(fname);
        }
    })
    .unwrap();

    pages.difference(&links).cloned().collect()
}
