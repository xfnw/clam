use crate::{html::Pages, Error};
use git2::{Oid, Repository, Time};
use orgize::ParseConfig;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    rc::Rc,
};

pub type HistMap = HashMap<PathBuf, HistMeta>;

pub struct HistMeta {
    pub create_time: Time,
    pub modify_time: Time,
    pub creator: String,
    pub last_editor: String,
    pub last_msg: Option<String>,
    pub contributors: HashSet<String>,
}

pub fn make_time_tree(repo: &Repository, oid: Oid) -> Result<HistMap, Error> {
    macro_rules! add_times {
        ($time_a:expr, $time_c:expr, $message:expr, $author:expr, $diff:expr, $metadata:expr) => {
            for change in $diff.deltas() {
                let path = change.new_file().path().ok_or(Error::BadGitPath)?;
                if let Some(entry) = $metadata.get_mut(path) {
                    if !entry.contributors.contains($author) {
                        entry.contributors.insert($author.to_string());
                    }
                    if entry.modify_time < $time_c {
                        entry.modify_time = $time_c.clone();
                        entry.last_editor = $author.to_string();
                        entry.last_msg = $message.clone();
                    }
                    if entry.create_time > $time_a {
                        entry.create_time = $time_a.clone();
                        entry.creator = $author.to_string();
                    }
                } else {
                    let mut contributors = HashSet::new();
                    contributors.insert($author.to_string());
                    $metadata.insert(
                        path.to_owned(),
                        HistMeta {
                            create_time: $time_a.clone(),
                            modify_time: $time_c.clone(),
                            creator: $author.to_string(),
                            last_editor: $author.to_string(),
                            last_msg: $message.clone(),
                            contributors,
                        },
                    );
                }
            }
        };
    }

    let mut revwalk = repo.revwalk()?;
    revwalk.push(oid)?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut metadata: HistMap = HashMap::new();

    for cid in revwalk {
        let commit = repo.find_commit(cid?)?;
        let tree = commit.tree()?;
        let parents = commit.parent_count();
        let message = commit.message().map(str::to_string);
        let author = commit.author();
        let time_a = author.when();
        let time_c = commit.time();
        let author = author.name().ok_or(Error::BadAuthor)?;

        // initial commit, everything touched
        if parents == 0 {
            let diff = repo.diff_tree_to_tree(None, Some(&tree), None)?;
            add_times!(time_a, time_c, message, author, diff, metadata);
            continue;
        }

        for parent in 0..parents {
            let ptree = commit.parent(parent)?.tree()?;
            let diff = repo.diff_tree_to_tree(Some(&ptree), Some(&tree), None)?;
            add_times!(time_a, time_c, message, author, diff, metadata);
        }
    }

    Ok(metadata)
}

pub fn walk_callback(
    repo: &Repository,
    dir: &str,
    entry: &git2::TreeEntry,
    org_cfg: &ParseConfig,
    pages: &mut Pages,
    links: &mut HashMap<PathBuf, Vec<Rc<PathBuf>>>,
) -> Result<(), Error> {
    let object = entry.to_object(repo)?;
    let name = entry.name().ok_or(Error::NonUTF8Path)?;

    let Ok(blob) = object.into_blob() else {
        // is probably a directory
        fs::create_dir_all(format!("{dir}{name}/")).map_err(Error::Dir)?;
        return Ok(());
    };

    if 0o120_000 == entry.filemode() {
        return Err(Error::SkipSymlink(format!("{dir}{name}")));
    }

    crate::html::generate_page(dir, name, blob.content(), org_cfg, pages, links)?;

    Ok(())
}
