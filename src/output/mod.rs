use chrono::NaiveDateTime;
use orgize::{rowan::NodeOrToken, Org, SyntaxNode, SyntaxToken};
use slugify::slugify;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub mod gmi;
pub mod html;

pub type TokenList = Vec<NodeOrToken<SyntaxNode, SyntaxToken>>;
pub type Pages = HashMap<PathBuf, (String, PathBuf, PageKeywords, String)>;

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
