use crate::{config::FeedConfig, git::ModifyMap, helpers::URL_PATH_UNSAFE, Error};
use chrono::{DateTime, NaiveDateTime};
use html_escaper::Escape;
use percent_encoding::utf8_percent_encode;
use regex::RegexSet;
use std::{
    cmp::min,
    collections::HashMap,
    fmt, fs,
    io::Write,
    path::{Component, PathBuf},
};

#[derive(boilerplate::Boilerplate)]
pub struct FeedXml<'a> {
    pub title: &'a str,
    pub id: &'a str,
    pub url: &'a str,
    pub path: &'a str,
    pub numdir: usize,
    pub updated: &'a AtomDateTime,
    pub entries: &'a [&'a AtomEntry<'a>],
}

#[derive(Debug)]
pub struct AtomEntry<'a> {
    pub title: &'a str,
    pub path: &'a str,
    pub author: &'a str,
    pub summary: Option<&'a str>,
    pub content: Option<&'a str>,
    pub updated: AtomDateTime,
}

/// `NaiveDateTime` that `Display`s to an atom feed compatible date (iso8601/rfc3339 subset)
/// without unnecessary allocation, as chrono gates iso8601 output behind the `alloc` feature
#[derive(Debug)]
pub struct AtomDateTime(pub NaiveDateTime);

impl AtomDateTime {
    /// create a new `AtomDateTime` from a unix timestamp
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
) -> Result<Vec<AtomEntry<'a>>, Error> {
    let mut entries = vec![];

    for (path, (title, old, _)) in titles {
        let Some(path) = path.to_str() else {
            continue;
        };

        let (updated, author, summary) = mtime.get(old).ok_or(Error::NoModifyTime)?;
        let updated = AtomDateTime::new(updated.seconds()).ok_or(Error::BadModifyTime)?;
        let summary = summary.as_deref();

        entries.push(AtomEntry {
            title,
            path,
            author,
            summary,
            content: None,
            updated,
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
) -> Result<(), Error> {
    if feed.path.components().any(|s| {
        matches!(
            s,
            Component::RootDir | Component::ParentDir | Component::CurDir
        )
    }) {
        return Err(Error::BadFeedPath);
    }
    let Some(path) = feed.path.to_str() else {
        return Err(Error::NonUTF8Path);
    };

    let include = if let Some(e) = &feed.include {
        RegexSet::new(e).map_err(Error::BadRegex)?
    } else {
        RegexSet::new([r"."]).map_err(Error::BadRegex)?
    };
    let exclude = if let Some(e) = &feed.exclude {
        RegexSet::new(e).map_err(Error::BadRegex)?
    } else {
        RegexSet::empty()
    };

    let filt: Vec<_> = entries
        .iter()
        .filter(|e| include.is_match(e.path) && !exclude.is_match(e.path))
        .collect();
    let numdir = feed.path.iter().count();

    let output = FeedXml {
        title: &feed.title,
        id,
        url,
        path,
        numdir,
        updated: head_updated(&filt).ok_or(Error::EmptyFeed)?,
        entries: &filt[..min(filt.len(), 42)],
    }
    .to_string();
    let mut f = fs::File::create(&feed.path).map_err(Error::File)?;
    f.write_all(output.as_bytes()).map_err(Error::File)?;
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
            content: None,
        };
        let entry2 = AtomEntry {
            title: "",
            path: "",
            author: "",
            updated: AtomDateTime::new(1169707221).unwrap(),
            summary: None,
            content: None,
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
            content: None,
        };
        let entries = [&entry];
        let feed = FeedXml {
            title: "🦊 feed",
            id: "tag:foxes.invalid,2024-12-13:foxfeed",
            url: "https://foxes.invalid",
            path: "foxfeed.xml",
            numdir: 6,
            updated: &AtomDateTime::new(1734117526).unwrap(),
            entries: &entries,
        }
        .to_string();

        assert_eq!(
            feed,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<?xml-stylesheet type="text/xsl" href="../../../../../style.xsl"?>
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
