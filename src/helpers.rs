use orgize::{Org, ast::Link, rowan::ast::AstNode};
use percent_encoding::{AsciiSet, CONTROLS};
use regex::RegexSet;
use serde::{Deserialize, Deserializer};
use slugify::slugify;
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
    let fileroot = Url::from_file_path(
        Path::new(
            #[cfg(windows)]
            "H:/",
            #[cfg(not(windows))]
            "/",
        )
        .join(name),
    )
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
        let fullpath = fullpath
            .strip_prefix(
                #[cfg(windows)]
                "H:/",
                #[cfg(not(windows))]
                "/",
            )
            .unwrap();
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
        let path = &link.path();
        let Ok(url) = (if let Some(p) = path.strip_prefix('*') {
            base.join(&format!("#{}", slugify!(p)))
        } else {
            base.join(path)
        }) else {
            continue;
        };
        callback(url);
    }
}

pub fn de_regex_set<'de, D>(deserializer: D) -> Result<RegexSet, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{Error, SeqAccess, Visitor, value};

    struct DeRegexSet;

    impl<'de> Visitor<'de> for DeRegexSet {
        type Value = RegexSet;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("set of string")
        }
        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
            A::Error: Error,
        {
            let regexes: Vec<String> =
                Deserialize::deserialize(value::SeqAccessDeserializer::new(seq))?;
            RegexSet::new(regexes).map_err(Error::custom)
        }
    }

    deserializer.deserialize_any(DeRegexSet)
}
