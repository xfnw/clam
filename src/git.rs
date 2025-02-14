use crate::{html::Pages, Error};
use git2::{Oid, Repository, Time};
use orgize::ParseConfig;
use std::{collections::HashMap, fs, path::PathBuf, rc::Rc};

pub type CreateMap = HashMap<PathBuf, (Time, String)>;
pub type ModifyMap = HashMap<PathBuf, (Time, String, Option<String>)>;

pub fn make_time_tree(repo: &Repository, oid: Oid) -> Result<(CreateMap, ModifyMap), Error> {
    macro_rules! add_times {
        ($time_a:expr, $time_c:expr, $message:expr, $author:expr, $diff:expr, $create_time:expr, $modify_time:expr) => {
            for change in $diff.deltas() {
                let path = change.new_file().path().ok_or(Error::BadGitPath)?;
                if let Some(entry) = $modify_time.get_mut(path) {
                    if entry.0 < $time_c {
                        entry.0 = $time_c.clone();
                        entry.1 = $author.to_string();
                        entry.2 = $message.clone();
                    }
                } else {
                    $modify_time.insert(
                        path.to_owned(),
                        ($time_c.clone(), $author.to_string(), $message.clone()),
                    );
                }
                if let Some(entry) = $create_time.get_mut(path) {
                    if entry.0 > $time_a {
                        entry.0 = $time_a.clone();
                        entry.1 = $author.to_string();
                    }
                } else {
                    $create_time.insert(path.to_owned(), ($time_a.clone(), $author.to_string()));
                }
            }
        };
    }

    let mut revwalk = repo.revwalk().map_err(Error::Git)?;
    revwalk.push(oid).map_err(Error::Git)?;
    revwalk.set_sorting(git2::Sort::TIME).map_err(Error::Git)?;

    let mut create_time: CreateMap = HashMap::new();
    let mut modify_time: ModifyMap = HashMap::new();

    for cid in revwalk {
        let commit = repo
            .find_commit(cid.map_err(Error::Git)?)
            .map_err(Error::Git)?;
        let tree = commit.tree().map_err(Error::Git)?;
        let parents = commit.parent_count();
        let message = commit.message().map(str::to_string);
        let author = commit.author();
        let time_a = author.when();
        let time_c = commit.time();
        let author = author.name().ok_or(Error::BadAuthor)?;

        // initial commit, everything touched
        if parents == 0 {
            let diff = repo
                .diff_tree_to_tree(None, Some(&tree), None)
                .map_err(Error::Git)?;
            add_times!(
                time_a,
                time_c,
                message,
                author,
                diff,
                create_time,
                modify_time
            );
            continue;
        }

        for parent in 0..parents {
            let ptree = commit
                .parent(parent)
                .map_err(Error::Git)?
                .tree()
                .map_err(Error::Git)?;
            let diff = repo
                .diff_tree_to_tree(Some(&ptree), Some(&tree), None)
                .map_err(Error::Git)?;
            add_times!(
                time_a,
                time_c,
                message,
                author,
                diff,
                create_time,
                modify_time
            );
        }
    }

    Ok((create_time, modify_time))
}

pub fn walk_callback(
    repo: &Repository,
    dir: &str,
    entry: &git2::TreeEntry,
    org_cfg: &ParseConfig,
    titles: &mut Pages,
    links: &mut HashMap<PathBuf, Vec<Rc<PathBuf>>>,
) -> Result<(), Error> {
    let object = entry.to_object(repo).map_err(Error::Git)?;
    let name = entry.name().ok_or(Error::NonUTF8Path)?;

    let Ok(blob) = object.into_blob() else {
        // is probably a directory
        fs::create_dir_all(format!("{dir}{name}/")).map_err(Error::Dir)?;
        return Ok(());
    };

    if 0o120_000 == entry.filemode() {
        return Err(Error::SkipSymlink(format!("{dir}{name}")));
    }

    crate::html::generate_page(dir, name, blob.content(), org_cfg, titles, links)?;

    Ok(())
}
