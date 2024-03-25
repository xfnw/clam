use git2::{Oid, Repository, Time};

use std::{collections::BTreeMap, error::Error, path::PathBuf};

pub type CreateMap = BTreeMap<PathBuf, (Time, String)>;
pub type ModifyMap = BTreeMap<PathBuf, Time>;

pub fn make_time_tree(
    repo: &Repository,
    oid: Oid,
) -> Result<(CreateMap, ModifyMap), Box<dyn Error>> {
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

pub fn walk_callback(
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
