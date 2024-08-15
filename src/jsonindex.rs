use git2::{Blob, Object, Repository};
use orgize::{Org, SyntaxKind};
use rowan::{ast::AstNode, NodeOrToken::Token};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct Entry {
    title: String,
    path: PathBuf,
    content: String,
}

pub fn print_index(repo: &Repository, commit: Object) {
    let commit = commit.into_commit().unwrap();
    let tree = commit.tree().unwrap();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        let Ok(obj) = entry.to_object(repo) else {
            return 0;
        };
        let Ok(blob) = obj.into_blob() else { return 0 };
        if 0o120000 == entry.filemode() {
            return 0;
        }
        let name = entry.name().unwrap();
        let fname: PathBuf = format!("/{dir}{}", name).into();
        if let Some(true) = fname.extension().map(|e| e == "org") {
            let entry = get_entry(fname, blob);
            println!("{}", serde_json::to_string(&entry).unwrap());
        }
        0
    })
    .unwrap();
}

fn get_entry(path: PathBuf, blob: Blob) -> Entry {
    let fstr = std::str::from_utf8(blob.content()).unwrap();
    let res = Org::parse(fstr);
    let document = res.document();
    let title = document.title().unwrap_or_else(|| "untitled".to_string());
    let syntax = document.syntax();
    let mut contents = vec![];

    for descendant in syntax.descendants_with_tokens() {
        let Token(token) = descendant else { continue };
        if token.kind() == SyntaxKind::TEXT {
            contents.push(token.text().to_string());
        }
    }

    Entry {
        title,
        path,
        content: contents.join(" "),
    }
}
