use crate::{
    default_org_cfg,
    output::{gmi::GmiExport, infer_title},
    util::map_files,
    OutputFormat, RepoArgs,
};
use git2::{Blob, Commit, Repository};
use serde::Serialize;
use std::{ffi::OsStr, path::PathBuf};

#[derive(Serialize)]
struct Entry {
    title: String,
    path: PathBuf,
    content: String,
}

pub fn print_index(repo: &Repository, commit: &Commit, args: &RepoArgs) {
    map_files(repo, commit, |name, blob| {
        let Some(entry) = (match name
            .extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("org") => get_entry_org(name, &blob, args.format),
            Some("htm" | "html") => return,
            _ => get_entry_raw(name, &blob),
        }) else {
            return;
        };
        println!("{}", serde_json::to_string(&entry).unwrap());
    })
    .unwrap();
}

fn get_entry_org(mut path: PathBuf, blob: &Blob, outfmt: OutputFormat) -> Option<Entry> {
    path.set_extension(match outfmt {
        OutputFormat::Html => "html",
        OutputFormat::Gmi => "gmi",
    });

    let fstr = std::str::from_utf8(blob.content()).ok()?;
    let res = default_org_cfg().parse(fstr);
    let title = res.title().unwrap_or_else(|| infer_title(&path));
    let mut export = GmiExport::default();
    res.traverse(&mut export);

    Some(Entry {
        title,
        path,
        content: export.finish(),
    })
}

fn get_entry_raw(path: PathBuf, blob: &Blob) -> Option<Entry> {
    let fstr = str::from_utf8(blob.content()).ok()?;
    let title = infer_title(&path);

    Some(Entry {
        title,
        path,
        content: fstr.to_string(),
    })
}
