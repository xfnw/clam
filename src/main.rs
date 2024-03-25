use chrono::{offset::Utc, DateTime, Datelike};
use clap::Parser;
use git2::{Oid, Repository, Time};
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
    author: &'a str,
    created: DateTime<Utc>,
    modified: DateTime<Utc>,
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
    repo: &Repository,
    dir_map: &BTreeMap<String, Vec<(String, Vec<u8>)>>,
    short_id: &str,
    // FIXME: needing both a short_id and oid is pretty silly, however git2
    // annoyingly does not provide an easy way to derive one from the other
    oid: Oid,
) -> Result<(), Box<dyn Error>> {
    let (ctime, mtime) = make_time_tree(repo, oid)?;

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

type CreateMap = BTreeMap<PathBuf, (Time, String)>;
type ModifyMap = BTreeMap<PathBuf, Time>;

fn make_time_tree(repo: &Repository, oid: Oid) -> Result<(CreateMap, ModifyMap), Box<dyn Error>> {
    macro_rules! add_times {
        ($time:expr, $author:expr, $diff:expr, $create_time:expr, $modify_time:expr) => {
            for change in $diff.deltas() {
                let path = change.new_file().path().ok_or("broken path")?;
                if let Some(entry) = $modify_time.get_mut(path) {
                    if *entry < $time {
                        *entry = $time.clone();
                    }
                } else {
                    $modify_time.insert(path.to_owned(), $time.clone());
                }
                if let Some(entry) = $create_time.get_mut(path) {
                    if entry.0 > $time {
                        entry.0 = $time.clone();
                        entry.1 = $author.to_string();
                    }
                } else {
                    $create_time.insert(path.to_owned(), ($time.clone(), $author.to_string()));
                }
            }
        };
    }

    let mut revwalk = repo.revwalk()?;
    revwalk.push(oid)?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut create_time: CreateMap = BTreeMap::new();
    let mut modify_time: ModifyMap = BTreeMap::new();

    for cid in revwalk {
        let commit = repo.find_commit(cid?)?;
        let tree = commit.tree()?;
        let parents = commit.parent_count();
        let author = commit.author();
        let time = author.when();
        let author = author.name().ok_or("broken author")?;

        // initial commit, everything touched
        if parents == 0 {
            let diff = repo.diff_tree_to_tree(None, Some(&tree), None)?;
            add_times!(time, author, diff, create_time, modify_time);
            continue;
        }

        for parent in 0..parents {
            let ptree = commit.parent(parent)?.tree()?;
            let diff = repo.diff_tree_to_tree(Some(&ptree), Some(&tree), None)?;
            add_times!(time, author, diff, create_time, modify_time);
        }
    }

    Ok((create_time, modify_time))
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
        walk_callback(&repo, dir, entry, &mut dir_map).unwrap();
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
