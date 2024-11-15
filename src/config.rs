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
    #[serde(default)]
    pub show_navigation: bool,
    #[serde(default)]
    pub feed: Vec<FeedConfig>,
}

#[derive(Deserialize, Debug)]
pub struct FeedConfig {
    pub title: String,
    pub path: String,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct OverrideConfig {
    pub url: Option<String>,
}

pub fn handle_config(
    titles: &HashMap<PathBuf, (String, PathBuf, Org)>,
    mtime: &ModifyMap,
    overrides: OverrideConfig,
) -> Option<ClamConfig> {
    let config = fs::read_to_string(".clam.toml").ok()?;
    let mut config: ClamConfig = toml_edit::de::from_str(&config).ok()?;
    let url = overrides.url.unwrap_or(config.url);
    let id = config.id.as_ref().unwrap_or(&url);

    if !config.feed.is_empty() {
        let entries = atom::entries(titles, mtime).ok()?;

        for feed in &config.feed {
            match atom::write_feed(feed, id, &url, entries.as_slice()) {
                Ok(_) => (),
                Err(e) => eprintln!("skipping {}: {}", feed.path, e),
            };
        }
    }

    config.url = url;

    Some(config)
}
