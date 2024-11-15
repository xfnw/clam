use clap::{Args, Parser, Subcommand};
use git2::{Commit, Repository};
use orgize::config::{ParseConfig, UseSubSuperscript};
use std::{collections::HashMap, error::Error, fs, io::Write, path::PathBuf};

mod atom;
mod config;
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
    /// override base url in feeds
    #[arg(long)]
    url: Option<String>,
    /// override inlining css
    #[arg(long)]
    inline: Option<bool>,
}

#[cfg(feature = "util")]
#[derive(Debug, Args)]
struct PreviewArgs {
    #[arg(default_value = "[::]:8086")]
    bindhost: std::net::SocketAddr,
}

static STYLESHEET_STR: &str = include_str!("style.css");
static STYLESHEET: &[u8] = STYLESHEET_STR.as_bytes();

fn generate(
    repo: &Repository,
    commit: Commit,
    overrides: config::OverrideConfig,
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

    let mut titles = HashMap::new();
    let mut links = HashMap::new();
    // TODO: get this stuff from .clam.toml or something
    let org_cfg = default_org_cfg();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Err(e) = git::walk_callback(repo, dir, entry, &org_cfg, &mut titles, &mut links) {
            eprintln!("{}", e);
        }
        0
    })?;

    let config = config::handle_config(&titles, &mtime, overrides);
    if config.is_none() {
        eprintln!("configless, no feeds generated and overrides ignored");
    }

    html::write_org_page(&titles, &ctime, &mtime, &links, short_id, config.as_ref())?;

    Ok(())
}

fn main() {
    let opt = Opt::parse();

    match &opt.command {
        Commands::Build(args) => open_repo(args, |r, c| do_build(r, c, args)),
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

fn do_build(repo: &Repository, commit: Commit, args: &RepoArgs) {
    let overrides = config::OverrideConfig {
        url: args.url.clone(),
        inline: args.inline,
    };

    generate(repo, commit, overrides).unwrap();
}

#[cfg(feature = "util")]
fn do_preview(args: &PreviewArgs) {
    let org_cfg = default_org_cfg();
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

fn default_org_cfg() -> ParseConfig {
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
