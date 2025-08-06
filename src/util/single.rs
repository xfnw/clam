use crate::{
    Error, STYLESHEET_STR,
    git::HistMap,
    helpers::org_links,
    output::{Page, PageKeywords, get_keywords, infer_title},
};
use git2::{Commit, Repository};
use html_escaper::{Escape, Trusted};
use orgize::{
    ParseConfig,
    export::{Container, Event, HtmlEscape, Traverser},
};
use slugify::slugify;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    rc::Rc,
};
use url::Url;

#[derive(boilerplate::Boilerplate)]
struct SingleHtml<'a> {
    entries: &'a [Entry<'a>],
}

struct Entry<'a> {
    title: &'a str,
    slug: &'a str,
    body: &'a str,
}

struct LinkSlugExport {
    myurl: Url,
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

                let path = slug_url(path, &self.myurl);

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

fn slug_url(url: impl AsRef<str>, current: &Url) -> String {
    let url = url.as_ref();
    if let Some(f) = url.strip_prefix('*') {
        return format!("#{}", slugify!(f));
    }
    if let Ok(url) = current.join(url) {
        if url.scheme() == "file" {
            return if let Some(f) = url.fragment() {
                format!("#{f}")
            } else {
                let slug = slugify!(url.path());
                let mindex = if url.path().ends_with('/') {
                    "-index-org"
                } else {
                    ""
                };
                format!("#{slug}{mindex}")
            };
        }
    }
    url.to_string()
}

fn generate_page(
    dir: &str,
    name: &str,
    file: &[u8],
    org_cfg: &ParseConfig,
    pages: &mut HashMap<String, Page>,
    links: &mut HashMap<PathBuf, Vec<Rc<String>>>,
) -> Result<(), Error> {
    let full_path = format!("{dir}{name}");
    let old_path = PathBuf::from(&full_path);
    let bpath = Path::new("/").join(&old_path);
    let page = if bpath
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("org"))
    {
        let fstr = str::from_utf8(file).map_err(Error::NonUTF8Org)?;
        let res = org_cfg.clone().parse(fstr);
        let title = res.title().unwrap_or_else(|| infer_title(&bpath));
        let keywords = get_keywords(&res);

        let myslug = Rc::new(slugify!(&full_path));
        org_links(&res, &bpath, |l| {
            let l = l.to_owned();

            if let Some(e) = links.get_mut(&l) {
                e.push(myslug.clone());
            } else {
                links.insert(l, vec![myslug.clone()]);
            }
        });

        let mut html_export = LinkSlugExport {
            myurl: Url::from_file_path(&bpath).unwrap(),
            exp: crate::output::html::Handler::default(),
        };
        res.traverse(&mut html_export);
        let body = html_export.exp.exp.finish();

        Page {
            title,
            old_path,
            keywords,
            body,
        }
    } else {
        let Ok(fstr) = str::from_utf8(file) else {
            return Ok(());
        };
        let title = infer_title(&bpath);
        let body = format!("<pre>{}</pre>", HtmlEscape(&fstr));
        Page {
            title,
            old_path,
            keywords: PageKeywords::default(),
            body,
        }
    };

    if pages.insert(slugify!(&full_path), page).is_some() {
        // grumble grumble insert does not give back key ownership >:(
        return Err(Error::SlugExists(slugify!(&full_path)));
    }

    Ok(())
}

fn generate_entry<'a>(
    slug: &'a str,
    page: &'a Page,
    _links: &'a HashMap<String, Vec<Rc<String>>>,
    _hist: &'a HistMap,
) -> Entry<'a> {
    let Page {
        title,
        old_path: _,
        keywords: _,
        body,
    } = page;
    Entry { title, slug, body }
}

pub fn print_html(repo: &Repository, commit: &Commit) {
    let tree = commit.tree().unwrap();
    let hmeta = crate::git::make_time_tree(repo, commit.id()).unwrap();
    let org_cfg = crate::default_org_cfg();
    let mut pages = HashMap::new();
    let mut links = HashMap::new();

    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Err(e) = crate::git::walk_callback(repo, dir, entry, |name, blob| {
            generate_page(dir, name, blob.content(), &org_cfg, &mut pages, &mut links)
        }) {
            eprintln!("{e}");
        }
        0
    })
    .unwrap();

    let entries: Vec<_> = pages
        .iter()
        .map(|(slug, page)| generate_entry(slug, page, &links, &hmeta))
        .collect();

    println!("{}", SingleHtml { entries: &entries });
}
