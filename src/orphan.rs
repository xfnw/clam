use git2::{Blob, Object, Repository};
use orgize::{ast::Link, Org};
use rowan::ast::AstNode;
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

pub fn get_orphans(repo: &Repository, commit: Object) -> BTreeSet<PathBuf> {
    let commit = commit.into_commit().unwrap();
    let tree = commit.tree().unwrap();
    let mut pages = BTreeSet::new();
    let mut links = BTreeSet::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        let Ok(obj) = entry.to_object(repo) else {
            return 0;
        };
        let Ok(blob) = obj.into_blob() else { return 0 };
        if 0o120000 == entry.filemode() {
            return 0;
        }
        let name: PathBuf = format!("/{dir}{}", entry.name().unwrap()).into();
        if let Some(true) = name.extension().map(|e| e == "org") {
            find_links(&name, blob, &mut links);
            pages.insert(name);
        }
        0
    })
    .unwrap();

    pages.difference(&links).cloned().collect()
}

fn find_links(name: &Path, blob: Blob, links: &mut BTreeSet<PathBuf>) {
    let fstr = std::str::from_utf8(blob.content()).unwrap();
    let res = Org::parse(fstr);
    let document = res.document();
    let syntax = document.syntax();
    for descendant in syntax.descendants() {
        let Some(link) = Link::cast(descendant) else {
            continue;
        };
        let path = link.path();
        let path = match path.split_once('#') {
            Some((p, _)) => p,
            None => &path,
        };
        let parent = name.parent().expect("borked name");
        let fullpath = parent.join(path);
        let fullpath = normalize(&fullpath);
        links.insert(fullpath);
    }
}

// why is this not a thing in the std???
// https://github.com/rust-lang/rfcs/issues/2208
fn normalize(path: &Path) -> PathBuf {
    let mut res = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) => panic!("no windows"),
            Component::RootDir => res.push("/"),
            Component::CurDir => (),
            Component::ParentDir => {
                res.pop();
            }
            Component::Normal(n) => res.push(n),
        }
    }

    res
}
