use crate::{atom, git::ModifyMap};
use orgize::Org;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Deserialize, Debug)]
pub struct ClamConfig {
    pub id: Option<String>,
    pub url: String,
    pub extra_header: Option<String>,
    pub extra_footer: Option<String>,
    pub feed: Option<Vec<FeedConfig>>,
}

#[derive(Deserialize, Debug)]
pub struct FeedConfig {
    pub title: String,
    pub path: String,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

pub fn handle_config(
    titles: &HashMap<PathBuf, (String, PathBuf, Org)>,
    mtime: &ModifyMap,
    override_url: Option<&str>,
) -> Option<ClamConfig> {
    let config = fs::read_to_string(".clam.toml").ok()?;
    let config: ClamConfig = toml_edit::de::from_str(&config).ok()?;
    if let Some(ref feeds) = config.feed {
        let entries = atom::entries(titles, mtime).ok()?;
        let id = config.id.as_ref().unwrap_or(&config.url);
        let url = override_url.unwrap_or(&config.url);

        for feed in feeds {
            match atom::write_feed(feed, id, url, entries.as_slice()) {
                Ok(_) => (),
                Err(e) => eprintln!("skipping {}: {}", feed.path, e),
            };
        }
    }

    Some(config)
}
