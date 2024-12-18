use crate::{config::FeedConfig, git::ModifyMap, shared::URL_PATH_UNSAFE};
use chrono::{DateTime, NaiveDateTime};
use html_escaper::Escape;
use percent_encoding::utf8_percent_encode;
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

pub fn head_updated<'a>(entries: &'a [&'a AtomEntry<'a>]) -> Option<&'a AtomDateTime> {
    Some(&entries.first()?.updated)
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
        updated: head_updated(&filt).ok_or("no entries in feed")?,
        entries: &filt[..min(filt.len(), 42)],
    }
    .to_string();
    let mut f = fs::File::create(&feed.path)?;
    f.write_all(output.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::atom::*;

    #[test]
    fn check_updated() {
        assert!(head_updated(&[]).is_none());

        let entry1 = AtomEntry {
            title: "",
            path: "",
            author: "",
            updated: AtomDateTime::new(1633462756).unwrap(),
            summary: None,
        };
        let entry2 = AtomEntry {
            title: "",
            path: "",
            author: "",
            updated: AtomDateTime::new(1169707221).unwrap(),
            summary: None,
        };
        let entries = [&entry1, &entry2];

        assert_eq!(
            head_updated(&entries).unwrap().to_string(),
            "2021-10-05T19:39:16Z"
        );
    }

    #[test]
    fn snapshot_feed() {
        let entry = AtomEntry {
            title: "hi – there",
            path: "🦊.html",
            author: "fox",
            updated: AtomDateTime::new(1734116293).unwrap(),
            summary: Some("did you know that foxes—which are very fluffy—exist?"),
        };
        let entries = [&entry];
        let feed = FeedXml {
            title: "🦊 feed",
            id: "tag:foxes.invalid,2024-12-13:foxfeed",
            url: "https://foxes.invalid",
            path: "foxfeed.xml",
            updated: &AtomDateTime::new(1734117526).unwrap(),
            entries: &entries,
        }
        .to_string();

        assert_eq!(
            feed,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
<title>🦊 feed</title>
<id>tag:foxes.invalid,2024-12-13:foxfeed/foxfeed.xml</id>
<link rel="self" href="https://foxes.invalid/foxfeed.xml"/>
<updated>2024-12-13T19:18:46Z</updated>
<entry>
<title>hi – there</title>
<id>tag:foxes.invalid,2024-12-13:foxfeed/%F0%9F%A6%8A.html</id>
<link rel="alternate" href="https://foxes.invalid/%F0%9F%A6%8A.html"/>
<author><name>fox</name></author>
<updated>2024-12-13T18:58:13Z</updated>
<summary>did you know that foxes—which are very fluffy—exist?</summary>
</entry>
</feed>
"#
        );
    }
}
