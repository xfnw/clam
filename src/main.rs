#![allow(clippy::too_many_arguments)]

use clap::{Args, Parser, Subcommand};
use git2::{Commit, Repository};
use orgize::config::{ParseConfig, UseSubSuperscript};
use serde::Deserialize;
use std::{collections::HashMap, error::Error, fs, io::Write, path::PathBuf};

mod atom;
mod git;
mod html;
mod shared;
#[cfg(feature = "util")]
mod util;

#[derive(Debug, Parser)]
struct Opt {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// generate site from git repository
    Build(RepoArgs),
    /// serve the current directory in limited preview mode
    #[cfg(feature = "util")]
    Preview(PreviewArgs),
    /// check for orphan pages
    #[cfg(feature = "util")]
    Orphan(RepoArgs),
    /// output page content as json lines
    #[cfg(feature = "util")]
    Jsonindex(RepoArgs),
    /// output links between pages in graphviz dot format
    #[cfg(feature = "util")]
    Dot(RepoArgs),
}

#[derive(Debug, Args)]
struct RepoArgs {
    #[arg(required = true)]
    repository: PathBuf,

    #[arg(default_value = "HEAD")]
    branch: String,
}

#[cfg(feature = "util")]
#[derive(Debug, Args)]
struct PreviewArgs {
    #[arg(default_value = "[::]:8086")]
    bindhost: std::net::SocketAddr,
}

#[derive(Deserialize, Debug)]
struct ClamConfig {
    id: Option<String>,
    url: String,
    feed: Option<Vec<FeedConfig>>,
}

#[derive(Deserialize, Debug)]
struct FeedConfig {
    title: String,
    path: String,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
}

static STYLESHEET: &[u8] = include_bytes!("style.css");

fn generate(
    org_cfg: &ParseConfig,
    repo: &Repository,
    commit: Commit,
) -> Result<(), Box<dyn Error>> {
    let short_id = commit.as_object().short_id().unwrap();
    let short_id = short_id.as_str().unwrap();
    let oid = commit.id();
    let tree = commit.tree().unwrap();

    let (ctime, mtime) = git::make_time_tree(repo, oid)?;

    {
        let mut f = fs::File::create("style.css")?;
        f.write_all(STYLESHEET)?;
    }

    let year_ago = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)?
        .as_secs()
        - 365 * 24 * 60 * 60;
    let year_ago: i64 = year_ago.try_into()?;
    let mut titles = HashMap::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Err(e) = git::walk_callback(
            repo,
            dir,
            entry,
            org_cfg,
            &ctime,
            &mtime,
            year_ago,
            short_id,
            &mut titles,
        ) {
            eprintln!("{}", e);
        }
        0
    })?;

    if let Ok(config) = fs::read_to_string(".clam.toml") {
        let config: ClamConfig = toml_edit::de::from_str(&config)?;
        if let Some(feeds) = config.feed {
            let entries = atom::entries(&titles, &mtime)?;
            let id = config.id.as_ref().unwrap_or(&config.url);

            for feed in feeds {
                match atom::write_feed(&feed, id, &config.url, entries.as_slice()) {
                    Ok(_) => (),
                    Err(e) => eprintln!("skipping {}: {}", feed.path, e),
                };
            }

            return Ok(());
        }
    }

    eprintln!("no configured feeds, skipping");

    Ok(())
}

fn main() {
    let opt = Opt::parse();

    match &opt.command {
        Commands::Build(args) => open_repo(args, do_build),
        #[cfg(feature = "util")]
        Commands::Preview(args) => do_preview(args),
        #[cfg(feature = "util")]
        Commands::Orphan(args) => open_repo(args, do_orphan),
        #[cfg(feature = "util")]
        Commands::Jsonindex(args) => open_repo(args, util::jsonindex::print_index),
        #[cfg(feature = "util")]
        Commands::Dot(args) => open_repo(args, util::dot::print_dot),
    }
}

fn do_build(repo: &Repository, commit: Commit) {
    let org_cfg = org_cfg();

    generate(&org_cfg, repo, commit).unwrap();
}

#[cfg(feature = "util")]
fn do_preview(args: &PreviewArgs) {
    let org_cfg = org_cfg();
    util::preview::serve(&org_cfg, args.bindhost);
}

#[cfg(feature = "util")]
fn do_orphan(repo: &Repository, commit: Commit) {
    let orphans = util::orphan::get_orphans(repo, commit);

    for o in orphans.into_iter() {
        println!("{}", o.display());
    }
}

fn open_repo<F>(args: &RepoArgs, callback: F)
where
    F: Fn(&Repository, Commit),
{
    let repo = Repository::open(&args.repository).unwrap();
    let commit = repo.revparse_single(&args.branch).unwrap();
    let commit = commit.into_commit().unwrap();
    callback(&repo, commit);
}

fn org_cfg() -> ParseConfig {
    // TODO: get this stuff from .clam.toml or something
    ParseConfig {
        todo_keywords: (
            ["TODO", "PENDING", "DELAYED", "RERUN"]
                .map(|s| s.to_string())
                .to_vec(),
            ["DONE", "RESOLVED", "FIXED", "WONTFIX"]
                .map(|s| s.to_string())
                .to_vec(),
        ),
        use_sub_superscript: UseSubSuperscript::Brace,
        ..Default::default()
    }
}
