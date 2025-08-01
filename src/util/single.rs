use git2::{Commit, Repository};
use html_escaper::{Escape, Trusted};
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
