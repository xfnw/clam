use super::{find_links, map_org};
use git2::{Commit, Repository};
use std::{collections::HashSet, path::PathBuf};

fn get_orphans(repo: &Repository, commit: &Commit) -> HashSet<PathBuf> {
    let mut pages = HashSet::new();
    let mut links = HashSet::new();

    map_org(repo, commit, |name, blob| {
        find_links(&name, &blob, |l| {
            links.insert(l.to_owned());
        });
        pages.insert(name);
    })
    .unwrap();

    pages.difference(&links).cloned().collect()
}

pub fn print_orphans(repo: &Repository, commit: &Commit) {
    let orphans = get_orphans(repo, commit);

    for o in orphans {
        println!("{}", o.display());
    }
}
