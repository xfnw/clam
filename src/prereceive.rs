use crate::{Error, PreReceiveArgs};
use git2::{Delta, Oid, Repository};
use regex::RegexSet;
use std::io;

#[derive(Debug)]
pub enum Action {
    Create,
    Delete,
    Modify,
}

#[derive(Debug)]
pub struct Rules {
    require_signing: bool,
    no_deletion: bool,
    no_creation: bool,
    allow_pattern: RegexSet,
    protect_pattern: RegexSet,
}

impl Rules {
    pub fn from_args(args: &PreReceiveArgs) -> Result<Self, Error> {
        let allow_pattern = RegexSet::new(&args.allow_pattern)?;
        let protect_pattern = RegexSet::new(&args.protect_pattern)?;
        let res = Self {
            require_signing: args.require_signing,
            no_deletion: args.no_deletion,
            no_creation: args.no_creation,
            allow_pattern,
            protect_pattern,
        };
        Ok(res)
    }

    pub fn signed(&self, is_signed: bool) -> Result<(), Error> {
        if self.require_signing && !is_signed {
            return Err(Error::NotSigned);
        }
        Ok(())
    }

    pub fn check(&self, path: &str, action: &Action) -> Result<(), Error> {
        match action {
            Action::Create => {
                if self.no_creation {
                    return Err(Error::BadCreate(path.to_string()));
                }
            }
            Action::Delete => {
                if self.no_deletion {
                    return Err(Error::BadDelete(path.to_string()));
                }
            }
            Action::Modify => {}
        }

        if !self.allow_pattern.is_match(path) {
            return Err(Error::NotAllowed(path.to_string()));
        }
        if self.protect_pattern.is_match(path) {
            return Err(Error::Protected(path.to_string()));
        }

        Ok(())
    }
}

fn check_commit(repo: &Repository, rules: &Rules, cid: Oid) -> Result<(), Error> {
    let is_signed = repo.extract_signature(&cid, None).is_ok();
    rules.signed(is_signed)?;
    let commit = repo.find_commit(cid)?;
    let tree = commit.tree()?;
    let parents = commit.parent_count();

    if parents == 0 {
        let diff = repo.diff_tree_to_tree(None, Some(&tree), None)?;
        check_deltas(rules, &diff)?;
    }

    for parent in 0..parents {
        let ptree = commit.parent(parent)?.tree()?;
        let diff = repo.diff_tree_to_tree(Some(&ptree), Some(&tree), None)?;
        check_deltas(rules, &diff)?;
    }

    Ok(())
}

fn check_deltas(rules: &Rules, diff: &git2::Diff<'_>) -> Result<(), Error> {
    for change in diff.deltas() {
        let Some(Ok(path)) = change.new_file().path_bytes().map(std::str::from_utf8) else {
            return Err(Error::NonUTF8Path);
        };
        let action = match change.status() {
            Delta::Added => Action::Create,
            Delta::Deleted => Action::Delete,
            Delta::Modified => Action::Modify,
            _ => unreachable!(),
        };

        rules.check(path, &action)?;
    }

    Ok(())
}

fn handle(args: &PreReceiveArgs) -> Result<(), Error> {
    let rules = Rules::from_args(args)?;
    let repo = Repository::open_from_env()?;
    let stdin = io::stdin().lines();

    for line in stdin {
        let line = line.map_err(Error::Stdin)?;
        let split: Vec<_> = line.split(' ').collect();
        let [old, new, refname] = split[..] else {
            return Err(Error::InvalidHookInput);
        };

        let old = Oid::from_str(old)?;
        let new = Oid::from_str(new)?;
        if old.is_zero() || new.is_zero() {
            return Err(Error::CreateRef(refname.to_string()));
        }

        ensure_reachable(&rules, &repo, old, new)?;
    }

    Ok(())
}

fn ensure_reachable(rules: &Rules, repo: &Repository, old: Oid, new: Oid) -> Result<(), Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
    revwalk.push(new)?;

    for cid in revwalk {
        let cid = cid?;
        if cid == old {
            return Ok(());
        }
        check_commit(repo, rules, cid)?;
    }

    Err(Error::ForcePush)
}

pub fn hook(args: &PreReceiveArgs) {
    if let Err(e) = handle(args) {
        println!("rejecting push: {e}");
        std::process::exit(1);
    }
}
