use crate::Error;
use git2::{Oid, Repository, Time};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

#[derive(Debug)]
pub struct HistMeta {
    pub create_time: Time,
    pub modify_time: Time,
    pub creator: String,
    pub last_editor: String,
    pub last_commit: String,
    pub last_msg: Option<String>,
    pub contributors: HashSet<String>,
}

pub fn make_time_tree(repo: &Repository, oid: Oid) -> Result<HashMap<PathBuf, HistMeta>, Error> {
    macro_rules! add_times {
        ($time_a:expr, $time_c:expr, $message:expr, $author:expr, $committer:expr, $short_id:expr, $diff:expr, $metadata:expr) => {
            for change in $diff.deltas() {
                let path = change.new_file().path().ok_or(Error::BadGitPath)?;
                if let Some(entry) = $metadata.get_mut(path) {
                    if !entry.contributors.contains($author) {
                        entry.contributors.insert($author.to_string());
                    }
                    if !entry.contributors.contains($committer) {
                        entry.contributors.insert($committer.to_string());
                    }
                    if entry.modify_time < $time_c {
                        entry.modify_time = $time_c.clone();
                        entry.last_editor = $author.to_string();
                        entry.last_commit = $short_id.to_string();
                        entry.last_msg = $message.clone();
                    }
                    if entry.create_time > $time_a {
                        entry.create_time = $time_a.clone();
                        entry.creator = $author.to_string();
                    }
                } else {
                    let mut contributors = HashSet::new();
                    contributors.insert($author.to_string());
                    if $author != $committer {
                        contributors.insert($committer.to_string());
                    }
                    $metadata.insert(
                        path.to_owned(),
                        HistMeta {
                            create_time: $time_a.clone(),
                            modify_time: $time_c.clone(),
                            creator: $author.to_string(),
                            last_editor: $author.to_string(),
                            last_commit: $short_id.to_string(),
                            last_msg: $message.clone(),
                            contributors,
                        },
                    );
                }
            }
        };
    }

    let mailmap = repo.mailmap()?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push(oid)?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut metadata: HashMap<PathBuf, HistMeta> = HashMap::new();

    for cid in revwalk {
        let commit = repo.find_commit(cid?)?;
        let short_id = commit.as_object().short_id()?;
        let short_id = short_id.as_str().unwrap();
        let tree = commit.tree()?;
        let parents = commit.parent_count();
        let message = commit.message().map(str::to_string);
        let author = commit.author();
        let author = mailmap.resolve_signature(&author).unwrap_or(author);
        let committer = commit.committer();
        let committer = mailmap.resolve_signature(&committer).unwrap_or(committer);
        let time_a = author.when();
        let time_c = commit.time();
        let author = author.name().ok_or(Error::BadAuthor)?;
        let committer = committer.name().ok_or(Error::BadCommitter)?;

        // initial commit, everything touched
        if parents == 0 {
            let diff = repo.diff_tree_to_tree(None, Some(&tree), None)?;
            add_times!(
                time_a, time_c, message, author, committer, short_id, diff, metadata
            );
            continue;
        }

        for parent in 0..parents {
            let ptree = commit.parent(parent)?.tree()?;
            let diff = repo.diff_tree_to_tree(Some(&ptree), Some(&tree), None)?;
            add_times!(
                time_a, time_c, message, author, committer, short_id, diff, metadata
            );
        }
    }

    Ok(metadata)
}

pub fn walk_callback<F>(
    repo: &Repository,
    dir: &str,
    entry: &git2::TreeEntry,
    create: bool,
    callback: F,
) -> Result<(), Error>
where
    F: FnOnce(&str, git2::Blob) -> Result<(), Error>,
{
    let name = entry.name().ok_or(Error::NonUTF8Path)?;

    match entry.filemode() {
        // normal files
        0o100_644 | 0o100_755 => (),
        // directories
        0o040_000 => {
            if create {
                fs::create_dir_all(format!("{dir}{name}/")).map_err(Error::Dir)?;
            }
            return Ok(());
        }
        // symlinks
        0o120_000 => {
            return Err(Error::SkipSymlink(format!("{dir}{name}")));
        }
        // git submodules
        0o160_000 => {
            return Err(Error::SkipSubmodule(format!("{dir}{name}")));
        }
        any => eprintln!("unknown filemode {any:o} for {dir}{name}"),
    }

    let object = entry.to_object(repo)?;
    let blob = object.into_blob().map_err(|_| Error::NotABlob)?;

    callback(name, blob)
}
