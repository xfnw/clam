use crate::PreReceiveArgs;
use git2::{Delta, Oid, Repository};
use regex::RegexSet;
use std::{fmt, io};

#[derive(Debug)]
pub enum Action {
    Create,
    Delete,
    Modify,
}

#[derive(Debug)]
pub enum Error {
    InvalidInput,
    ForcePush,
    NonUTF8Path,
    NotSigned,
    BadDelete(String),
    BadCreate(String),
    NotAllowed(String),
    Protected(String),
    CreateRef(String),
    BadRegex(regex::Error),
    Stdin(io::Error),
    Git(git2::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidInput => {
                write!(f, "invalid input. this is being used as a git hook, yes?")
            }
            Error::ForcePush => write!(f, "force-pushes are not permitted"),
            Error::NonUTF8Path => write!(f, "paths that are not utf-8 are not supported"),
            Error::NotSigned => write!(f, "signing your commits is required"),
            Error::BadDelete(p) => write!(f, "deleting pages is not permitted: {p}"),
            Error::BadCreate(p) => write!(f, "creating pages is not permitted: {p}"),
            Error::NotAllowed(p) => write!(f, "editing this page is not permitted: {p}"),
            Error::Protected(p) => write!(f, "page is protected: {p}"),
            Error::CreateRef(r) => write!(f, "creating new refs is not permitted: {r}"),
            Error::BadRegex(e) => write!(f, "failed to compile regex: {e}"),
            Error::Stdin(e) => write!(f, "failed to read stdin: {e}"),
            Error::Git(e) => write!(f, "internal git error: {e}"),
        }
    }
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
    pub fn from_args(args: &PreReceiveArgs) -> Result<Rules, Error> {
        let allow_pattern = RegexSet::new(&args.allow_pattern).map_err(Error::BadRegex)?;
        let protect_pattern = RegexSet::new(&args.protect_pattern).map_err(Error::BadRegex)?;
        let res = Rules {
            require_signing: args.require_signing,
            no_deletion: args.no_deletion,
            no_creation: args.no_creation,
            allow_pattern,
            protect_pattern,
        };
        Ok(res)
    }

    pub fn signed(&self, signed: bool) -> Result<(), Error> {
        if self.require_signing && !signed {
            return Err(Error::NotSigned);
        }
        Ok(())
    }

    pub fn check(&self, path: &str, action: Action) -> Result<(), Error> {
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
        };

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
    let commit = repo.find_commit(cid).map_err(Error::Git)?;
    let tree = commit.tree().map_err(Error::Git)?;
    let parents = commit.parent_count();

    for parent in 0..parents {
        let ptree = commit
            .parent(parent)
            .map_err(Error::Git)?
            .tree()
            .map_err(Error::Git)?;
        let diff = repo
            .diff_tree_to_tree(Some(&ptree), Some(&tree), None)
            .map_err(Error::Git)?;

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

            rules.check(path, action)?;
        }
    }

    Ok(())
}

fn handle(args: &PreReceiveArgs) -> Result<(), Error> {
    let rules = Rules::from_args(args)?;
    let repo = Repository::open_from_env().map_err(Error::Git)?;
    let stdin = io::stdin().lines();

    for line in stdin {
        let line = line.map_err(Error::Stdin)?;
        let split: Vec<_> = line.split(' ').collect();
        let [old, new, refname] = split[..] else {
            return Err(Error::InvalidInput);
        };

        let old = Oid::from_str(old).map_err(Error::Git)?;
        let new = Oid::from_str(new).map_err(Error::Git)?;
        if old.is_zero() || new.is_zero() {
            return Err(Error::CreateRef(refname.to_string()));
        }

        let mut revwalk = repo.revwalk().map_err(Error::Git)?;
        revwalk.push(new).map_err(Error::Git)?;
        revwalk.hide(old).map_err(Error::Git)?;

        let mut visited = false;
        for cid in revwalk {
            visited = true;
            let cid = cid.map_err(Error::Git)?;
            check_commit(&repo, &rules, cid)?;
        }
        if !visited {
            return Err(Error::ForcePush);
        }
    }

    Ok(())
}

pub fn hook(args: &PreReceiveArgs) {
    if let Err(e) = handle(args) {
        println!("rejecting push: {}", e);
        std::process::exit(1);
    }
}
