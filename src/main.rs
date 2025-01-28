use clap::{Args, Parser, Subcommand};
use foxerror::FoxError;
use git2::{Commit, Repository};
use orgize::config::{ParseConfig, UseSubSuperscript};
use std::{collections::HashMap, env::set_current_dir, fs, io::Write, path::PathBuf};

mod atom;
mod config;
mod git;
mod helpers;
mod html;
mod prereceive;
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
    /// hook for filtering incoming git pushes
    PreReceive(PreReceiveArgs),
}

#[derive(Debug, Args)]
struct RepoArgs {
    #[arg(required = true)]
    repository: PathBuf,
    #[arg(default_value = "HEAD")]
    branch: String,
    /// change to directory after opening repository
    #[arg(short = 'C', long, value_name = "TARGET")]
    chdir: Option<PathBuf>,
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

#[derive(Debug, Args)]
struct PreReceiveArgs {
    /// require commits to be signed (does not verify signatures)
    #[arg(long)]
    require_signing: bool,
    /// do not allow any pages to be deleted
    #[arg(long)]
    no_deletion: bool,
    /// do not allow any new pages to be created
    #[arg(long)]
    no_creation: bool,
    /// require that paths of edited pages match this set of regexes
    ///
    /// may be specified multiple times for multiple patterns
    #[arg(long, value_name = "PATTERN", default_value = ".")]
    allow_pattern: Vec<String>,
    /// do not allow edits to paths matching this set of regexes
    ///
    /// may be specified multiple times for multiple patterns
    #[arg(long, value_name = "PATTERN")]
    protect_pattern: Vec<String>,
}

#[derive(Debug, FoxError)]
enum Error {
    /// invalid input. this is being used as a git hook, yes?
    InvalidHookInput,
    /// force-pushes are not permitted
    ForcePush,
    /// paths that are not utf-8 are not supported
    NonUTF8Path,
    /// signing your commits is required
    NotSigned,
    /// deleting pages is not permitted
    BadDelete(String),
    /// creating pages is not permitted
    BadCreate(String),
    /// editing this page is not permitted
    NotAllowed(String),
    /// page is protected
    Protected(String),
    /// creating new refs is not permitted
    CreateRef(String),
    /// failed to compile regex
    BadRegex(regex::Error),
    /// failed to read stdin
    Stdin(std::io::Error),
    /// internal git error
    Git(git2::Error),
    /// failed to write file
    File(std::io::Error),
    /// failed to write directory
    Dir(std::io::Error),
    /// your system clock is screwed
    Clock(std::time::SystemTimeError),
    /// missing creation time
    NoCreateTime,
    /// missing modification time
    NoModifyTime,
    /// creation time broken
    BadCreateTime,
    /// modification time broken
    BadModifyTime,
    /// stop using 300 billion year old software
    TimeOverflow,
    /// invalid feed path
    BadFeedPath,
    /// no entries in feed
    EmptyFeed,
    /// invalid path in git repository
    BadGitPath,
    /// broken author
    BadAuthor,
    /// skipping symlink
    SkipSymlink(String),
}

static STYLESHEET_STR: &str = include_str!("style.css");
static STYLESHEET: &[u8] = STYLESHEET_STR.as_bytes();
static STYLEFEED: &[u8] = include_bytes!("style.xsl");

fn generate(
    repo: &Repository,
    commit: &Commit,
    overrides: config::OverrideConfig,
) -> Result<(), Error> {
    let short_id = commit.as_object().short_id().unwrap();
    let short_id = short_id.as_str().unwrap();
    let oid = commit.id();
    let tree = commit.tree().unwrap();

    let (ctime, mtime) = git::make_time_tree(repo, oid)?;

    {
        let mut f = fs::File::create("style.css").map_err(Error::File)?;
        f.write_all(STYLESHEET).map_err(Error::File)?;
        let mut f = fs::File::create("style.xsl").map_err(Error::File)?;
        f.write_all(STYLEFEED).map_err(Error::File)?;
    }

    let mut titles = HashMap::new();
    let mut links = HashMap::new();
    // TODO: get this stuff from .clam.toml or something
    let org_cfg = default_org_cfg();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Err(e) = git::walk_callback(repo, dir, entry, &org_cfg, &mut titles, &mut links) {
            eprintln!("{e}");
        }
        0
    })
    .map_err(Error::Git)?;

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
        Commands::PreReceive(args) => prereceive::hook(args),
    }
}

fn do_build(repo: &Repository, commit: &Commit, args: &RepoArgs) {
    let overrides = config::OverrideConfig {
        url: args.url.clone(),
        inline: args.inline,
    };

    if let Err(e) = generate(repo, commit, overrides) {
        eprintln!("failed to generate: {e}");
        std::process::exit(1);
    }
}

#[cfg(feature = "util")]
fn do_preview(args: &PreviewArgs) {
    let org_cfg = default_org_cfg();
    util::preview::serve(&org_cfg, args.bindhost);
}

#[cfg(feature = "util")]
fn do_orphan(repo: &Repository, commit: &Commit) {
    let orphans = util::orphan::get_orphans(repo, commit);

    for o in orphans {
        println!("{}", o.display());
    }
}

fn open_repo<F>(args: &RepoArgs, callback: F)
where
    F: Fn(&Repository, &Commit),
{
    let repo = Repository::open(&args.repository).unwrap();
    let commit = repo.revparse_single(&args.branch).unwrap();
    let commit = commit.into_commit().unwrap();

    if let Some(target) = &args.chdir {
        set_current_dir(target).expect("changing directory");
    }

    callback(&repo, &commit);
}

fn default_org_cfg() -> ParseConfig {
    ParseConfig {
        todo_keywords: (
            ["TODO", "PENDING", "DELAYED", "RERUN"]
                .map(str::to_string)
                .to_vec(),
            ["DONE", "RESOLVED", "FIXED", "WONTFIX"]
                .map(str::to_string)
                .to_vec(),
        ),
        use_sub_superscript: UseSubSuperscript::Brace,
        ..Default::default()
    }
}
