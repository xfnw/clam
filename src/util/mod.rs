use git2::{Blob, Commit, Repository};
use orgize::{ast::Link, Org};
use rowan::ast::AstNode;
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

pub mod jsonindex;
pub mod orphan;
pub mod preview;

/// add all links in an org document to a set.
///
/// will add mangled entries when encountering links to
/// external resources.
pub fn find_links(name: &Path, blob: Blob, links: &mut HashSet<PathBuf>) {
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
        let mut fullpath = normalize(&fullpath);
        match fullpath.extension().map(|e| e == "org") {
            Some(true) => (),
            _ => fullpath.push("index.org"),
        };
        links.insert(fullpath);
    }
}

/// normalize a path *without* checking actual files.
///
/// this may give incorrect answers when symlinks are
/// involved. use [`std::fs::canonicalize`] instead
/// when this is an issue.
///
/// panics if given a windows-style path prefix.
// why is this not a thing in the std???
// https://github.com/rust-lang/rfcs/issues/2208
pub fn normalize(path: &Path) -> PathBuf {
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

/// run a function on every org file in repository
pub fn map_org<F>(repo: &Repository, commit: Commit, mut callback: F) -> Result<(), git2::Error>
where
    F: FnMut(PathBuf, Blob),
{
    let tree = commit.tree()?;

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        let Ok(obj) = entry.to_object(repo) else {
            return 0;
        };
        let Ok(blob) = obj.into_blob() else { return 0 };
        if 0o120000 == entry.filemode() {
            return 0;
        }
        let name = entry.name().unwrap();
        let name: PathBuf = format!("/{dir}{}", name).into();
        if let Some(true) = name.extension().map(|e| e == "org") {
            callback(name, blob);
        }
        0
    })
}
