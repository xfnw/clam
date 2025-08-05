use chrono::NaiveDateTime;
use orgize::{Org, ParseConfig, SyntaxNode, SyntaxToken, ast::Token, rowan::NodeOrToken};
use slugify::slugify;
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    io::Write,
    path::Component,
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::{Error, OutputFormat, config::ClamConfig, git::HistMap};

pub mod gmi;
pub mod html;

pub type TokenList = Vec<NodeOrToken<SyntaxNode, SyntaxToken>>;
pub type Pages = HashMap<PathBuf, Page>;

pub struct Page {
    pub title: String,
    pub old_path: PathBuf,
    pub keywords: PageKeywords,
    pub html: String,
}

pub struct PageMetadata<'a> {
    pub author: &'a str,
    pub commit: &'a str,
    pub modified: NaiveDateTime,
    pub year: i32,
    pub incoming: Option<&'a [(&'a str, &'a str)]>,
    pub footer: Option<&'a str>,
    pub contributors: usize,
}

#[derive(Default)]
pub struct PageKeywords {
    pub author: Option<String>,
    pub language: Option<String>,
    pub year: Option<String>,
}

pub fn infer_title(filename: &Path) -> String {
    let Some(stem) = filename.file_stem().and_then(OsStr::to_str) else {
        return "untitled".to_string();
    };
    slugify!(stem, separator = " ")
}

pub fn get_keywords(res: &Org) -> PageKeywords {
    macro_rules! match_keywords {
        ($k:ident, $kw:ident, ($($key:ident),*)) => {
            match $k.key().to_ascii_lowercase().as_ref() {
                $(stringify!($key) => {
                    if $kw.$key.is_none() {
                        $kw.$key = Some($k.value().trim().to_string());
                    }
                })*
                _ => {}
            }
        }
    }
    let mut keywords = PageKeywords::default();
    for k in res.keywords() {
        match_keywords!(k, keywords, (author, language, year));
    }
    keywords
}

pub fn write_org_page(
    format: OutputFormat,
    pages: &Pages,
    hist: &HistMap,
    links: &HashMap<PathBuf, Vec<Rc<PathBuf>>>,
    short_id: &str,
    config: Option<&ClamConfig>,
) -> Result<(), Error> {
    match format {
        OutputFormat::Html => html::write_org_page(pages, hist, links, short_id, config),
        OutputFormat::Gmi => gmi::write_org_page(pages, hist, links, short_id, config),
    }
}

pub fn generate_page(
    format: OutputFormat,
    dir: &str,
    name: &str,
    file: &[u8],
    org_cfg: &ParseConfig,
    pages: &mut Pages,
    links: &mut HashMap<PathBuf, Vec<Rc<PathBuf>>>,
) -> Result<(), Error> {
    match format {
        OutputFormat::Html => html::generate_page(dir, name, file, org_cfg, pages, links),
        OutputFormat::Gmi => gmi::generate_page(dir, name, file, org_cfg, pages, links),
    }
}

pub fn write_redirect_page(format: OutputFormat, path: &Path, target: &str) -> Result<(), Error> {
    if path.components().any(|s| {
        matches!(
            s,
            Component::RootDir | Component::ParentDir | Component::CurDir
        )
    }) {
        return Err(Error::UnsafePath);
    }

    let content = match format {
        OutputFormat::Html => html::write_redirect_page(path, target),
        OutputFormat::Gmi => gmi::write_redirect_page(path, target),
    };

    let mut f = File::create(path).map_err(Error::File)?;
    f.write_all(&content.into_bytes()).map_err(Error::File)
}

// FIXME: use an actual url parser
pub fn mangle_link(path: &Token, suffix: &str, asuffix: &str) -> String {
    let path = path.strip_prefix("file:").unwrap_or(path);
    if let Some(p) = path.strip_prefix('*') {
        let mut p = slugify!(p);
        p.insert(0, '#');
        return p;
    }
    if path.starts_with("//") || path.contains("://") {
        return path.to_string();
    }
    if let Some(p) = path.strip_suffix(".org") {
        let mut p = p.to_string();
        p.push_str(suffix);
        return p;
    }
    path.replace(".org#", asuffix)
}
