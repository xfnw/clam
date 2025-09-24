use super::{find_links, map_org};
use git2::{Commit, Repository};
use std::{collections::HashSet, path::PathBuf};

fn get_redlinks(repo: &Repository, commit: &Commit) -> HashSet<PathBuf> {
    let mut pages = HashSet::new();
    let mut links = HashSet::new();

    map_org(repo, commit, |name, blob| {
        find_links(&name, &blob, |l| {
            if l.extension().is_some_and(|e| e.eq_ignore_ascii_case("org")) {
                links.insert(l.to_owned());
            }
        });
        pages.insert(name);
    })
    .unwrap();

    links.difference(&pages).cloned().collect()
}

pub fn print_redlinks(repo: &Repository, commit: &Commit) {
    let redlinks = get_redlinks(repo, commit);

    for l in redlinks {
        println!("{}", l.display());
    }
}
