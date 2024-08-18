use super::{find_links, map_org};
use git2::{Object, Repository};
use std::{collections::HashSet, path::PathBuf};

pub fn get_orphans(repo: &Repository, commit: Object) -> HashSet<PathBuf> {
    let commit = commit.into_commit().unwrap();
    let mut pages = HashSet::new();
    let mut links = HashSet::new();

    map_org(repo, commit, |name, blob| {
        find_links(&name, blob, |l| {
            links.insert(l);
        });
        pages.insert(name);
    })
    .unwrap();

    pages.difference(&links).cloned().collect()
}
