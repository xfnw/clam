use crate::git::ModifyMap;
use chrono::{DateTime, NaiveDateTime};
use html_escaper::Escape;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write},
    path::PathBuf,
};

#[derive(boilerplate::Boilerplate)]
pub struct FeedXml<'a> {
    pub title: &'a str,
    pub id: &'a str,
    pub url: &'a str,
    pub updated: &'a AtomDateTime,
    pub entries: &'a [AtomEntry<'a>],
}

#[derive(Debug)]
pub struct AtomEntry<'a> {
    pub title: &'a str,
    pub path: &'a str,
    pub author: &'a str,
    pub updated: AtomDateTime,
}

/// NaiveDateTime that `Display`s to an atom feed compatible date (iso8601/rfc3339 subset)
/// without unnecessary allocation, as chrono gates iso8601 output behind the `alloc` feature
#[derive(Debug)]
pub struct AtomDateTime(pub NaiveDateTime);

impl AtomDateTime {
    /// create a new AtomDateTime from a unix timestamp
    pub fn new(unix: i64) -> Option<Self> {
        let ts = DateTime::from_timestamp(unix, 0)?;
        Some(Self(ts.naive_utc()))
    }
}

impl fmt::Display for AtomDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.date().fmt(f)?;
        f.write_char('T')?;
        self.0.time().fmt(f)?;
        f.write_char('Z')
    }
}

pub fn entries<'a>(
    titles: &'a BTreeMap<PathBuf, (String, PathBuf)>,
    mtime: &'a ModifyMap,
    exclude: &Option<BTreeSet<String>>,
) -> Result<Vec<AtomEntry<'a>>, Box<dyn std::error::Error>> {
    let mut entries = vec![];

    for (path, (title, old)) in titles.iter() {
        let path = match path.to_str() {
            Some(p) => p,
            None => continue,
        };

        if let Some(exclude) = exclude {
            if exclude.contains(path) {
                continue;
            }
        }

        let (updated, author) = mtime.get(old).ok_or("missing modification info")?;
        let updated = AtomDateTime::new(updated.seconds()).ok_or("broken modification date")?;

        entries.push(AtomEntry {
            title,
            path,
            author,
            updated,
        });
    }

    entries.sort_by(|x, y| y.updated.0.cmp(&x.updated.0));
    Ok(entries)
}
