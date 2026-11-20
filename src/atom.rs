use crate::{Error, config::FeedConfig, git::HistMeta, helpers::URL_PATH_UNSAFE, output::Page};
use percent_encoding::{CONTROLS, utf8_percent_encode};
use std::{
    cmp::min,
    collections::HashMap,
    fmt, fs,
    io::{Cursor, Write},
    path::{Component, PathBuf},
};
use time::{UtcDateTime, error::ComponentRange, format_description::well_known::Rfc3339};

#[derive(boilerplate::Boilerplate)]
struct FeedXml<'a> {
    title: &'a str,
    id: &'a str,
    url: &'a str,
    path: &'a str,
    numdir: usize,
    is_html: bool,
    updated: &'a AtomDateTime,
    entries: &'a [&'a AtomEntry<'a>],
}

#[derive(Debug)]
pub struct AtomEntry<'a> {
    title: &'a str,
    path: &'a str,
    author: &'a str,
    summary: Option<&'a str>,
    content: Option<&'a str>,
    published: AtomDateTime,
    updated: AtomDateTime,
}

/// [`UtcDateTime`] that [`Display`]s to an atom feed compatible date (iso8601/rfc3339 subset)
#[derive(Debug)]
pub struct AtomDateTime(pub UtcDateTime);

impl AtomDateTime {
    /// create a new `AtomDateTime` from a unix timestamp
    pub fn new(unix: i64) -> Result<Self, ComponentRange> {
        UtcDateTime::from_unix_timestamp(unix).map(Self)
    }
}

impl fmt::Display for AtomDateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // FIXME: replace this nonsense if time ever implements fmt::Write stuff.
        // it is on their breaking changes wishlist:
        // https://github.com/time-rs/time/issues/650
        let mut buf = Cursor::new([0u8; "9999-12-31T23:59:59.999999999Z".len()]);
        // not sure what the usize this returns is for, but it is NOT the written length,
        // hence needing to record that ourselves with the Cursor
        self.0
            .format_into(&mut buf, &Rfc3339)
            .map_err(|_| fmt::Error)?;
        #[expect(clippy::cast_possible_truncation)]
        f.write_str(
            str::from_utf8(&buf.get_ref()[..buf.position() as usize]).map_err(|_| fmt::Error)?,
        )
    }
}

/// replace most c0 control codes since xml 1.0 does not
/// allow them and does not have a way to escape them???
struct Clean<'a>(&'a str);

impl fmt::Display for Clean<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;
        for c in self.0.chars() {
            f.write_char(match c {
                '\x09' | '\x0a' | '\x0d' => c,
                '\0'..='\x1f' => '\u{fffd}',
                _ => c,
            })?;
        }
        Ok(())
    }
}

pub fn entries<'a>(
    pages: &'a HashMap<PathBuf, Page>,
    metadata: &'a HashMap<PathBuf, HistMeta>,
) -> Result<Vec<AtomEntry<'a>>, Error> {
    let mut entries = vec![];

    for (
        path,
        Page {
            title,
            old_path,
            body: html,
            ..
        },
    ) in pages
    {
        let Some(path) = path.to_str() else {
            continue;
        };

        let HistMeta {
            create_time,
            modify_time,
            last_editor,
            last_msg,
            ..
        } = metadata.get(old_path).ok_or(Error::MissingHist)?;
        let published = AtomDateTime::new(create_time.seconds()).map_err(Error::BadCreateTime)?;
        let updated = AtomDateTime::new(modify_time.seconds()).map_err(Error::BadModifyTime)?;
        let summary = last_msg.as_deref();
        let content = Some(html.as_ref());

        entries.push(AtomEntry {
            title,
            path,
            author: last_editor,
            summary,
            content,
            published,
            updated,
        });
    }

    entries.sort_by(|x, y| {
        y.updated
            .0
            .cmp(&x.updated.0)
            .then(y.published.0.cmp(&x.published.0))
            .then(y.path.cmp(x.path))
    });
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
    is_html: bool,
) -> Result<(), Error> {
    if feed.path.components().any(|s| {
        matches!(
            s,
            Component::RootDir | Component::ParentDir | Component::CurDir
        )
    }) {
        return Err(Error::UnsafePath);
    }
    let Some(path) = feed.path.to_str() else {
        return Err(Error::NonUTF8Path);
    };

    let filt: Vec<_> = entries.iter().filter(|e| feed.contains(e.path)).collect();
    let numdir = feed.path.iter().count();

    let output = FeedXml {
        title: &feed.title,
        id,
        url,
        path,
        numdir,
        is_html,
        updated: head_updated(&filt).ok_or(Error::EmptyFeed)?,
        entries: &filt[..min(filt.len(), feed.max_items)],
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
            published: AtomDateTime::new(1_777_240_612).unwrap(),
            updated: AtomDateTime::new(1_633_462_756).unwrap(),
            summary: None,
            content: None,
        };
        let entry2 = AtomEntry {
            title: "",
            path: "",
            author: "",
            published: AtomDateTime::new(1_777_240_642).unwrap(),
            updated: AtomDateTime::new(1_169_707_221).unwrap(),
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
            published: AtomDateTime::new(1_714_168_779).unwrap(),
            updated: AtomDateTime::new(1_734_116_293).unwrap(),
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
            is_html: true,
            updated: &AtomDateTime::new(1_734_117_526).unwrap(),
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
<published>2024-04-26T21:59:39Z</published>
<updated>2024-12-13T18:58:13Z</updated>
<summary>did you know that foxes—which are very fluffy—exist?</summary>
</entry>
</feed>
"#
        );
    }

    #[test]
    fn longest_date() {
        let max = UtcDateTime::MAX;
        let atom = AtomDateTime(max);
        assert_eq!(atom.to_string(), max.format(&Rfc3339).unwrap());
    }
}
