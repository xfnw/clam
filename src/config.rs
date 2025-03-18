use crate::{atom, git::ModifyMap, html::Pages};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Deserialize, Debug)]
pub struct ClamConfig {
    pub id: Option<String>,
    pub url: String,
    pub extra_header: Option<String>,
    pub extra_footer: Option<String>,
    #[serde(default)]
    pub show_navigation: bool,
    #[serde(default)]
    pub inline: bool,
    #[serde(default)]
    pub feed: Vec<FeedConfig>,
}

#[derive(Deserialize, Debug)]
pub struct FeedConfig {
    pub title: String,
    pub path: PathBuf,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct OverrideConfig {
    pub url: Option<String>,
    pub inline: Option<bool>,
}

pub fn handle_config(
    pages: &Pages,
    mtime: &ModifyMap,
    overrides: OverrideConfig,
) -> Option<ClamConfig> {
    let config = fs::read_to_string(".clam.toml").ok()?;
    let mut config: ClamConfig = match toml_edit::de::from_str(&config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("could not parse config: {e}");
            return None;
        }
    };
    let url = overrides.url.unwrap_or(config.url);
    let id = config.id.as_ref().unwrap_or(&url);

    if !config.feed.is_empty() {
        let entries = atom::entries(pages, mtime).ok()?;

        for feed in &config.feed {
            if let Err(e) = atom::write_feed(feed, id, &url, entries.as_slice()) {
                eprintln!("skipping {}: {}", feed.path.display(), e);
            };
        }
    }

    config.url = url;
    config.inline = overrides.inline.unwrap_or(config.inline);

    Some(config)
}
