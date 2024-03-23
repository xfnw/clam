use clap::Parser;
use git2::Repository;
use html_escaper::{Escape, Trusted};
use orgize::{ast::Keyword, ParseConfig};
use rowan::ast::{support, AstNode};
use std::{collections::BTreeMap, error::Error, fs, io::Write, path::PathBuf};

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
}

fn walk_callback(
    repo: &Repository,
    dir: &str,
    entry: &git2::TreeEntry,
    dir_map: &mut BTreeMap<String, Vec<(String, Vec<u8>)>>,
) -> Result<(), Box<dyn Error>> {
    let object = entry.to_object(repo)?;
    let name = entry.name().ok_or("invalid unicode in a file name")?;

    let blob = match object.into_blob() {
        Ok(blob) => blob,

        Err(_) => {
            // is probably a directory
            dir_map.insert(format!("{}{}/", dir, name), vec![]);
            return Ok(());
        }
    };

    let directory = dir_map.get_mut(dir).ok_or("VERBODEN TOEGANG")?;
    directory.push((name.to_string(), blob.content().to_vec()));

    Ok(())
}

fn generate(
    org_cfg: &ParseConfig,
    _repo: &Repository,
    dir_map: &BTreeMap<String, Vec<(String, Vec<u8>)>>,
    id: &str,
) -> Result<(), Box<dyn Error>> {
    for (dir, files) in dir_map.iter() {
        fs::create_dir_all(dir)?;

        for file in files.iter() {
            let mut full_path: PathBuf = format!("{}{}", dir, file.0).into();

            let pcontent: Option<Vec<u8>> =
                match full_path.extension().and_then(std::ffi::OsStr::to_str) {
                    Some("org") => {
                        full_path.set_extension("html");
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

                        let template = PageHtml {
                            title,
                            body: res.to_html(),
                            commit: id,
                        };
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
    let id = commit.short_id().unwrap();
    let id = id.as_str().unwrap();
    let commit = commit.into_commit().unwrap();
    let tree = commit.tree().unwrap();
    let mut dir_map = BTreeMap::new();
    dir_map.insert("".to_string(), vec![]);

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        walk_callback(&repo, dir, entry, &mut dir_map).unwrap();
        0
    })
    .unwrap();

    // TODO: get this stuff from clam.toml or something
    let org_cfg = ParseConfig {
        ..Default::default()
    };

    generate(&org_cfg, &repo, &dir_map, id).unwrap();
}
