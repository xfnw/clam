use crate::{Error, output::infer_title};
use git2::{Commit, Repository};
use html_escaper::{Escape, Trusted};
use orgize::{
    ParseConfig,
    export::{Container, Event, HtmlEscape, Traverser},
};
use slugify::slugify;
use std::{ffi::OsStr, path::PathBuf};
use url::Url;

#[derive(boilerplate::Boilerplate)]
struct SingleHtml<'a> {
    entries: &'a [Entry],
}

struct Entry {
    title: String,
    slug: String,
    body: String,
}

#[derive(Default)]
struct LinkSlugExport {
    exp: crate::output::html::Handler,
}

impl Traverser for LinkSlugExport {
    fn event(&mut self, event: orgize::export::Event, ctx: &mut orgize::export::TraversalContext) {
        match event {
            Event::Enter(Container::Link(ref link)) => {
                let path = link.path();

                // FIXME: would be nice to turn local images into data uris
                if path.starts_with("abbr:") || link.is_image() {
                    self.exp.event(event, ctx);
                    return;
                }

                let path = slug_url(path);

                self.exp
                    .exp
                    .push_str(format!("<a href=\"{}\">", HtmlEscape(&path)));

                if !link.has_description() {
                    self.exp.exp.push_str(format!("{}</a>", HtmlEscape(&path)));
                    ctx.skip();
                }
            }
            _ => self.exp.event(event, ctx),
        }
    }
}

fn slug_url(url: impl AsRef<str>) -> String {
    let url = url.as_ref();
    if let Some(f) = url.strip_prefix('*') {
        return format!("#{}", slugify!(f));
    }
    // grumble grumble url not having a consistent Err type
    // so i cant use .and_then()
    if let Ok(Ok(url)) = Url::from_directory_path("/").map(|u| u.join(url)) {
        if url.scheme() == "file" {
            return if let Some(f) = url.fragment() {
                format!("#{f}")
            } else {
                format!("#{}", slugify!(url.path()))
            };
        }
    }
    url.to_string()
}

fn generate_entry(
    dir: &str,
    name: &str,
    file: &[u8],
    org_cfg: &ParseConfig,
    entries: &mut Vec<Entry>,
) -> Result<(), Error> {
    let full_path = format!("{dir}{name}");
    let bpath = PathBuf::from(&full_path);
    let (title, body) = if bpath
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("org"))
    {
        let fstr = str::from_utf8(file).map_err(Error::NonUTF8Org)?;
        let res = org_cfg.clone().parse(fstr);
        let title = res.title().unwrap_or_else(|| infer_title(&bpath));

        let mut html_export = LinkSlugExport::default();
        res.traverse(&mut html_export);
        let body = html_export.exp.exp.finish();

        (title, body)
    } else {
        // TODO: include other kinds of files that are utf-8
        return Ok(());
    };

    entries.push(Entry {
        title,
        slug: slugify!(&full_path),
        body,
    });

    Ok(())
}

pub fn print_html(repo: &Repository, commit: &Commit) {
    let tree = commit.tree().unwrap();
    let mut entries = vec![];
    let org_cfg = crate::default_org_cfg();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Err(e) = crate::git::walk_callback(repo, dir, entry, |name, blob| {
            generate_entry(dir, name, blob.content(), &org_cfg, &mut entries)
        }) {
            eprintln!("{e}");
        }
        0
    })
    .unwrap();

    println!("{}", SingleHtml { entries: &entries });
}
