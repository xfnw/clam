use super::map_org;
use git2::{Blob, Commit, Repository};
use orgize::{
    export::{Container, Event, TraversalContext, Traverser},
    Org,
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct Entry {
    title: String,
    path: PathBuf,
    content: String,
}

#[derive(Default)]
struct TextExport {
    output: String,
}

impl TextExport {
    fn push_str(&mut self, s: impl AsRef<str>) {
        self.output += s.as_ref();
    }
    fn finish(self) -> String {
        self.output
    }
}

impl Traverser for TextExport {
    fn event(&mut self, event: Event, ctx: &mut TraversalContext) {
        match event {
            Event::Enter(Container::Keyword(_)) => ctx.skip(),
            Event::Enter(Container::Headline(headline)) => {
                for e in headline.title() {
                    self.element(e, ctx);
                }
                self.push_str("\n");
            }
            Event::Timestamp(time) => self.push_str(time.raw()),
            Event::Text(text) => self.push_str(text),
            _ => (),
        }
    }
}

pub fn print_index(repo: &Repository, commit: &Commit) {
    map_org(repo, commit, |mut name, blob| {
        name.set_extension("html");
        let entry = get_entry(name, &blob);
        println!("{}", serde_json::to_string(&entry).unwrap());
    })
    .unwrap();
}

fn get_entry(path: PathBuf, blob: &Blob) -> Entry {
    let fstr = std::str::from_utf8(blob.content()).unwrap();
    let res = Org::parse(fstr);
    let title = res.title().unwrap_or_else(|| "untitled".to_string());
    let mut export = TextExport::default();
    res.traverse(&mut export);

    Entry {
        title,
        path,
        content: export.finish(),
    }
}
