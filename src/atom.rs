use chrono::{offset::Utc, DateTime};
use html_escaper::Escape;

#[derive(boilerplate::Boilerplate)]
pub struct FeedXml<'a> {
    pub title: &'a str,
    pub id: &'a str,
    pub url: &'a str,
    pub updated: DateTime<Utc>,
    pub entries: Vec<AtomEntry<'a>>,
}

pub struct AtomEntry<'a> {
    pub title: &'a str,
    pub path: String,
    pub author: &'a str,
    pub updated: DateTime<Utc>,
}
