use crate::{git::ModifyMap, FeedConfig};
use chrono::{DateTime, NaiveDateTime};
use html_escaper::Escape;
use regex::RegexSet;
use std::{cmp::min, collections::HashMap, fmt, fs, io::Write, path::PathBuf};

#[derive(boilerplate::Boilerplate)]
pub struct FeedXml<'a> {
    pub title: &'a str,
    pub id: &'a str,
    pub url: &'a str,
    pub path: &'a str,
    pub updated: &'a AtomDateTime,
    pub entries: &'a [&'a AtomEntry<'a>],
}

#[derive(Debug)]
pub struct AtomEntry<'a> {
    pub title: &'a str,
    pub path: &'a str,
    pub author: &'a str,
    pub summary: Option<&'a str>,
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
        use std::fmt::Write;

        self.0.date().fmt(f)?;
        f.write_char('T')?;
        self.0.time().fmt(f)?;
        f.write_char('Z')
    }
}

pub fn entries<'a>(
    titles: &'a HashMap<PathBuf, (String, PathBuf, orgize::Org)>,
    mtime: &'a ModifyMap,
) -> Result<Vec<AtomEntry<'a>>, Box<dyn std::error::Error>> {
    let mut entries = vec![];

    for (path, (title, old, _)) in titles.iter() {
        let path = match path.to_str() {
            Some(p) => p,
            None => continue,
        };

        let (updated, author, summary) = mtime.get(old).ok_or("missing modification info")?;
        let updated = AtomDateTime::new(updated.seconds()).ok_or("broken modification date")?;
        let summary = summary.as_deref();

        entries.push(AtomEntry {
            title,
            path,
            author,
            updated,
            summary,
        });
    }

    entries.sort_by(|x, y| y.updated.0.cmp(&x.updated.0));
    Ok(entries)
}

pub fn write_feed(
    feed: &FeedConfig,
    id: &str,
    url: &str,
    entries: &[AtomEntry],
) -> Result<(), Box<dyn std::error::Error>> {
    if feed.path.starts_with('/') || feed.path.contains("./") {
        return Err("invalid feed path".into());
    }

    let include = if let Some(e) = &feed.include {
        RegexSet::new(e)?
    } else {
        RegexSet::new([r"."])?
    };
    let exclude = if let Some(e) = &feed.exclude {
        RegexSet::new(e)?
    } else {
        RegexSet::empty()
    };

    let filt: Vec<_> = entries
        .iter()
        .filter(|e| include.is_match(e.path) && !exclude.is_match(e.path))
        .collect();

    let output = FeedXml {
        title: &feed.title,
        id,
        url,
        path: &feed.path,
        updated: &filt.first().ok_or("no entries in feed")?.updated,
        entries: &filt[..min(filt.len(), 42)],
    }
    .to_string();
    let mut f = fs::File::create(&feed.path)?;
    f.write_all(output.as_bytes())?;
    Ok(())
}
