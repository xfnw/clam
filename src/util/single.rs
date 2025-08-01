use git2::{Commit, Repository};
use html_escaper::{Escape, Trusted};
use orgize::export::{Container, Event, HtmlEscape, Traverser};
use slugify::slugify;
use url::Url;

#[derive(boilerplate::Boilerplate)]
struct SingleHtml<'a> {
    entries: &'a [&'a Entry<'a>],
}

struct Entry<'a> {
    title: &'a str,
    slug: String,
    body: &'a str,
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

pub fn print_html(_repo: &Repository, _commit: &Commit) {
    todo!()
}
