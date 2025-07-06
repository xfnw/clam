use crate::{output::gmi::GmiExport, util::map_org};
use git2::{Blob, Commit, Repository};
use orgize::Org;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct Entry {
    title: String,
    path: PathBuf,
    content: String,
}

pub fn print_index(repo: &Repository, commit: &Commit) {
    map_org(repo, commit, |mut name, blob| {
        name.set_extension("html");
        let entry = get_entry(name, &blob);
        println!("{}", serde_json::to_string(&entry).unwrap());
    })
    .unwrap();
}

fn get_entry(path: PathBuf, blob: &Blob) -> Entry {
    let fstr = std::str::from_utf8(blob.content()).unwrap();
    let res = Org::parse(fstr);
    let title = res.title().unwrap_or_else(|| "untitled".to_string());
    let mut export = GmiExport::default();
    res.traverse(&mut export);

    Entry {
        title,
        path,
        content: export.finish(),
    }
}
