use crate::{
    output::{gmi::GmiExport, infer_title},
    util::map_files,
};
use git2::{Blob, Commit, Repository};
use orgize::Org;
use serde::Serialize;
use std::{ffi::OsStr, path::PathBuf};

#[derive(Serialize)]
struct Entry {
    title: String,
    path: PathBuf,
    content: String,
}

pub fn print_index(repo: &Repository, commit: &Commit) {
    map_files(repo, commit, |name, blob| {
        let entry = match name
            .extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("org") => get_entry_org(name, &blob),
            Some("txt") => get_entry_raw(name, &blob),
            _ => return,
        };
        println!("{}", serde_json::to_string(&entry).unwrap());
    })
    .unwrap();
}

fn get_entry_org(mut path: PathBuf, blob: &Blob) -> Entry {
    path.set_extension("html");

    let fstr = std::str::from_utf8(blob.content()).unwrap();
    let res = Org::parse(fstr);
    let title = res.title().unwrap_or_else(|| infer_title(&path));
    let mut export = GmiExport::default();
    res.traverse(&mut export);

    Entry {
        title,
        path,
        content: export.finish(),
    }
}

fn get_entry_raw(path: PathBuf, blob: &Blob) -> Entry {
    let fstr = str::from_utf8(blob.content()).unwrap();
    let title = infer_title(&path);

    Entry {
        title,
        path,
        content: fstr.to_string(),
    }
}
