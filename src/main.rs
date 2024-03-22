use clap::Parser;
use git2::Repository;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Opt {
    #[arg(required = true)]
    repository: PathBuf,

    #[arg(default_value = "HEAD")]
    branch: String,
}

fn main() {
    let opt = Opt::parse();

    let repo = Repository::open(opt.repository).unwrap();
    let commit = repo.revparse_single(&opt.branch).unwrap();
    let id = commit.short_id().unwrap();
    let id = id.as_str().unwrap();
    let commit = commit.into_commit().unwrap();
    let tree = commit.tree().unwrap();

    tree.walk(git2::TreeWalkMode::PostOrder, move |dir, entry| {
        dbg!(&id, dir, entry.name().unwrap());
        0
    })
    .unwrap();
}
