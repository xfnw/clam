use crate::git::ModifyMap;
use chrono::{DateTime, NaiveDateTime};
use html_escaper::Escape;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(boilerplate::Boilerplate)]
pub struct FeedXml<'a> {
    pub title: &'a str,
    pub id: &'a str,
    pub url: &'a str,
    pub updated: &'a NaiveDateTime,
    pub entries: &'a [AtomEntry<'a>],
}

#[derive(Debug)]
pub struct AtomEntry<'a> {
    pub title: &'a str,
    pub path: &'a str,
    pub author: &'a str,
    pub updated: NaiveDateTime,
}

pub fn entries<'a>(
    titles: &'a BTreeMap<PathBuf, (String, PathBuf)>,
    mtime: &'a ModifyMap,
) -> Result<Vec<AtomEntry<'a>>, Box<dyn std::error::Error>> {
    let mut entries = vec![];

    for (path, (title, old)) in titles.iter() {
        let path = match path.to_str() {
            Some(p) => p,
            None => continue,
        };
        let (updated, author) = mtime.get(old).ok_or("missing modification info")?;
        let updated = DateTime::from_timestamp(updated.seconds(), 0)
            .ok_or("broken modification date")?
            .naive_utc();

        entries.push(AtomEntry {
            title,
            path,
            author,
            updated,
        });
    }

    entries.sort_by(|x, y| y.updated.cmp(&x.updated));
    Ok(entries)
}
