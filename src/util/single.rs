use git2::{Repository, Commit};
use html_escaper::{Escape, Trusted};

#[derive(boilerplate::Boilerplate)]
struct SingleHtml<'a> {
    entries: &'a [&'a Entry<'a>],
}

struct Entry<'a> {
    title: &'a str,
    slug: String,
    body: &'a str,
}

pub fn print_html(_repo: &Repository, _commit: &Commit) {
    todo!()
}
