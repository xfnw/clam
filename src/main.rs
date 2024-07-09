#![allow(clippy::too_many_arguments)]

use clap::{Args, Parser, Subcommand};
use git2::{Object, Repository};
use orgize::config::{ParseConfig, UseSubSuperscript};
use regex::RegexSet;
use serde_derive::Deserialize;
use std::{cmp::min, collections::BTreeMap, error::Error, fs, io::Write, path::PathBuf};

mod atom;
mod git;
mod html;

#[derive(Debug, Parser)]
struct Opt {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Build(CommonArgs),
}

#[derive(Debug, Args)]
struct CommonArgs {
    #[arg(required = true)]
    repository: PathBuf,

    #[arg(default_value = "HEAD")]
    branch: String,
}

#[derive(Deserialize, Debug)]
struct ClamConfig {
    title: String,
    id: Option<String>,
    url: String,
    exclude: Option<Vec<String>>,
}

fn generate(
    org_cfg: &ParseConfig,
    repo: &Repository,
    commit: Object,
) -> Result<(), Box<dyn Error>> {
    let short_id = commit.short_id().unwrap();
    let short_id = short_id.as_str().unwrap();
    let commit = commit.into_commit().unwrap();
    let oid = commit.id();
    let tree = commit.tree().unwrap();

    let (ctime, mtime) = git::make_time_tree(repo, oid)?;

    {
        let mut f = fs::File::create("style.css")?;
        f.write_all(include_bytes!("style.css"))?;
    }

    let year_ago = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)?
        .as_secs()
        - 365 * 24 * 60 * 60;
    let year_ago: i64 = year_ago.try_into()?;
    let mut titles = BTreeMap::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        git::walk_callback(
            repo,
            dir,
            entry,
            org_cfg,
            &ctime,
            &mtime,
            year_ago,
            short_id,
            &mut titles,
        )
        .unwrap();
        0
    })?;

    if let Ok(config) = fs::read_to_string(".clam.toml") {
        let config: ClamConfig = toml_edit::de::from_str(&config)?;
        let exclude = if let Some(e) = config.exclude {
            RegexSet::new(e)?
        } else {
            RegexSet::empty()
        };

        let feed = atom::entries(&titles, &mtime, &exclude)?;

        let mut f = fs::File::create("feed.xml")?;
        f.write_all(
            atom::FeedXml {
                title: &config.title,
                id: config.id.as_ref().unwrap_or(&config.url),
                url: &config.url,
                updated: &feed.first().ok_or("no entries in feed")?.updated,
                entries: &feed[..min(feed.len(), 42)],
            }
            .to_string()
            .as_bytes(),
        )?;
    } else {
        eprintln!("missing config file, skipping feed.xml creation");
    }

    Ok(())
}

fn main() {
    let opt = Opt::parse();

    match &opt.command {
        Commands::Build(args) => do_build(args),
    }
}

fn do_build(args: &CommonArgs) {
    let repo = Repository::open(&args.repository).unwrap();
    let commit = repo.revparse_single(&args.branch).unwrap();

    // TODO: get this stuff from .clam.toml or something
    let org_cfg = ParseConfig {
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
    };

    generate(&org_cfg, &repo, commit).unwrap();
}
