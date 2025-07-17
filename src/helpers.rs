use orgize::{ast::Link, rowan::ast::AstNode, Org};
use percent_encoding::{AsciiSet, CONTROLS};
use std::path::Path;
use url::Url;

pub const URL_UNSAFE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'\\')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

pub const URL_PATH_UNSAFE: &AsciiSet = &URL_UNSAFE.add(b'#').add(b'?');

/// run a function on every link in a syntax tree, as a [`Path`]
///
/// will skip links to external resources, and adds `index.org` to links to directories, use
/// [`org_urls`] if you do not want that
pub fn org_links<F>(res: &Org, name: &Path, mut callback: F)
where
    F: FnMut(&Path),
{
    let fileroot = Url::from_file_path(Path::new("/").join(name))
        .expect("current path should fit in a file url");
    org_urls(res, &fileroot, |url| {
        if url.scheme() != "file" {
            return;
        }
        let Ok(mut fullpath) = url.to_file_path() else {
            return;
        };
        if url.path().ends_with('/') {
            fullpath.push("index.org");
        }
        let fullpath = fullpath.strip_prefix("/").unwrap();
        callback(fullpath);
    });
}

/// run a function on every link in a syntax tree, as a [`Url`]
pub fn org_urls<F>(res: &Org, base: &Url, mut callback: F)
where
    F: FnMut(Url),
{
    let document = res.document();
    let syntax = document.syntax();
    for descendant in syntax.descendants() {
        let Some(link) = Link::cast(descendant) else {
            continue;
        };
        let Ok(url) = base.join(&link.path()) else {
            continue;
        };
        callback(url);
    }
}
