use git2::Blob;
use orgize::{ast::Link, Org};
use rowan::ast::AstNode;
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

pub mod jsonindex;
pub mod orphan;
pub mod preview;

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
        let fullpath = normalize(&fullpath);
        links.insert(fullpath);
    }
}

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
