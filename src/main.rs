use chrono::{offset::Utc, DateTime, Datelike};
use clap::Parser;
use git2::{Oid, Repository};
use html_escaper::{Escape, Trusted};
use orgize::{ast::Keyword, ParseConfig};
use rowan::ast::{support, AstNode};
use std::{collections::BTreeMap, error::Error, fs, io::Write, path::PathBuf};

mod git;

#[derive(Debug, Parser)]
struct Opt {
    #[arg(required = true)]
    repository: PathBuf,

    #[arg(default_value = "HEAD")]
    branch: String,
}

#[derive(boilerplate::Boilerplate)]
struct PageHtml<'a> {
    title: String,
    body: String,
    commit: &'a str,
    author: &'a str,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
    numdir: usize,
}

fn generate(
    org_cfg: &ParseConfig,
    repo: &Repository,
    dir_map: &BTreeMap<String, Vec<(String, Vec<u8>)>>,
    short_id: &str,
    // FIXME: needing both a short_id and oid is pretty silly, however git2
    // annoyingly does not provide an easy way to derive one from the other
    oid: Oid,
) -> Result<(), Box<dyn Error>> {
    let (ctime, mtime) = git::make_time_tree(repo, oid)?;

    {
        let mut f = fs::File::create("style.css")?;
        f.write_all(include_bytes!("style.css"))?;
    }

    for (dir, files) in dir_map.iter() {
        fs::create_dir_all(dir)?;

        for file in files.iter() {
            let mut full_path: PathBuf = format!("{}{}", dir, file.0).into();

            let pcontent: Option<Vec<u8>> =
                match full_path.extension().and_then(std::ffi::OsStr::to_str) {
                    Some("org") => {
                        let fstr = std::str::from_utf8(file.1.as_slice())?;
                        let res = org_cfg.clone().parse(fstr);

                        // https://github.com/PoiScript/orgize/issues/70#issuecomment-1916068875
                        let mut title = "untitled".to_string();
                        if let Some(section) = res.document().section() {
                            for keyword in support::children::<Keyword>(section.syntax()) {
                                if keyword.key() == "TITLE" {
                                    title = keyword.value().trim().to_string();
                                }
                            }
                        };

                        let (created, author) =
                            ctime.get(&full_path).ok_or("missing creation time")?;
                        let modified = mtime.get(&full_path).ok_or("missing modification time")?;

                        let template = PageHtml {
                            title,
                            body: res.to_html(),
                            commit: short_id,
                            author,
                            created: DateTime::from_timestamp(created.seconds(), 0)
                                .ok_or("broken creation date")?,
                            modified: DateTime::from_timestamp(modified.seconds(), 0)
                                .ok_or("broken modification date")?,
                            numdir: full_path.iter().count(),
                        };

                        full_path.set_extension("html");

                        Some(template.to_string().into_bytes())
                    }
                    _ => None,
                };
            let content = match &pcontent {
                Some(c) => c,
                None => &file.1,
            };

            let mut f = fs::File::create(full_path)?;
            f.write_all(content)?;
        }
    }

    Ok(())
}

fn main() {
    let opt = Opt::parse();

    let repo = Repository::open(&opt.repository).unwrap();
    let commit = repo.revparse_single(&opt.branch).unwrap();
    let short_id = commit.short_id().unwrap();
    let short_id = short_id.as_str().unwrap();
    let commit = commit.into_commit().unwrap();
    let oid = commit.id();
    let tree = commit.tree().unwrap();
    let mut dir_map = BTreeMap::new();
    dir_map.insert("".to_string(), vec![]);

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        git::walk_callback(&repo, dir, entry, &mut dir_map).unwrap();
        0
    })
    .unwrap();

    // TODO: get this stuff from clam.toml or something
    let org_cfg = ParseConfig {
        todo_keywords: (
            ["TODO", "PENDING", "DELAYED", "RERUN"]
                .map(|s| s.to_string())
                .to_vec(),
            ["DONE", "RESOLVED", "FIXED"]
                .map(|s| s.to_string())
                .to_vec(),
        ),
        ..Default::default()
    };

    generate(&org_cfg, &repo, &dir_map, short_id, oid).unwrap();
}
