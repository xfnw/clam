use git2::{Blob, Commit, Repository};
use orgize::Org;
use std::path::{Path, PathBuf};

use crate::helpers::org_links;

pub mod dot;
pub mod jsonindex;
pub mod orphan;
pub mod preview;

/// run a function on every link in an org document
///
/// will give mangled paths when encountering links to
/// external resources.
pub fn find_links<F>(name: &Path, blob: Blob, callback: F)
where
    F: FnMut(PathBuf),
{
    let fstr = std::str::from_utf8(blob.content()).unwrap();
    let res = Org::parse(fstr);
    org_links(&res, name, callback);
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
        if 0o120_000 == entry.filemode() {
            return 0;
        }
        let name = entry.name().unwrap();
        let name: PathBuf = format!("{dir}{name}").into();
        if Some(true) == name.extension().map(|e| e == "org") {
            callback(name, blob);
        }
        0
    })
}
