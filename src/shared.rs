use orgize::{ast::Link, Org};
use percent_encoding::{AsciiSet, CONTROLS};
use rowan::ast::AstNode;
use std::path::{Component, Path, PathBuf};

pub const URL_UNSAFE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// run a function on every link in a syntax tree
///
/// will give mangled paths when encountering links to
/// external resources.
pub fn org_links<F>(res: &Org, name: &Path, mut callback: F)
where
    F: FnMut(PathBuf),
{
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
        callback(fullpath);
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
