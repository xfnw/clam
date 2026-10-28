use crate::{
    OutputFormat, atom,
    git::HistMeta,
    helpers::de_regex_set,
    output::{Page, mangle_link, replace_org_extension, write_redirect_page},
};
use regex::RegexSet;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

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
    pub use_redirects_file: bool,
    #[serde(default)]
    pub feed: Vec<FeedConfig>,
    #[serde(default)]
    pub redirect: Vec<RedirectConfig>,
}

#[derive(Deserialize, Debug)]
pub struct FeedConfig {
    pub title: String,
    pub path: PathBuf,
    #[serde(default = "full_regex_set", deserialize_with = "de_regex_set")]
    pub include: RegexSet,
    #[serde(default = "RegexSet::empty", deserialize_with = "de_regex_set")]
    pub exclude: RegexSet,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
}

impl FeedConfig {
    pub fn contains(&self, path: &str) -> bool {
        self.include.is_match(path) && !self.exclude.is_match(path)
    }
}

// serde is annoying
const fn default_max_items() -> usize {
    42
}

fn full_regex_set() -> RegexSet {
    RegexSet::new(["."]).unwrap()
}

#[derive(Deserialize, Debug)]
pub struct RedirectConfig {
    pub path: PathBuf,
    pub target: String,
}

#[derive(Debug)]
pub struct OverrideConfig {
    pub url: Option<String>,
    pub inline: Option<bool>,
    pub format: OutputFormat,
    pub use_redirects_file: Option<bool>,
}

pub fn handle_config(
    pages: &HashMap<PathBuf, Page>,
    metadata: &HashMap<PathBuf, HistMeta>,
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

    let id = config.id.unwrap_or_else(|| config.url.clone());
    let url = overrides.url.unwrap_or(config.url);

    if !config.feed.is_empty() {
        let entries = atom::entries(pages, metadata).ok()?;

        for feed in &config.feed {
            if let Err(e) = atom::write_feed(
                feed,
                &id,
                &url,
                entries.as_slice(),
                matches!(overrides.format, OutputFormat::Html),
            ) {
                eprintln!("skipping {}: {}", feed.path.display(), e);
            }
        }
    }

    if overrides
        .use_redirects_file
        .unwrap_or(config.use_redirects_file)
    {
        if fs::File::create("_redirects")
            .ok()
            .map(BufWriter::new)
            .map(|mut w| {
                config
                    .redirect
                    .iter()
                    .map(|RedirectConfig { path, target }| {
                        let path = replace_org_extension(path, overrides.format);
                        let target = mangle_link(target, overrides.format);
                        w.write_all(b"/")
                            .and_then(|()| w.write_all(path.as_os_str().as_encoded_bytes()))
                            .and_then(|()| w.write_all(b"\t"))
                            .and_then(|()| w.write_all(target.as_bytes()))
                            .and_then(|()| w.write_all(b"\n"))
                            .ok()
                    })
                    .collect::<Option<()>>()
            })
            .is_none()
        {
            eprintln!("failed to write _redirects file");
        }
    } else {
        for RedirectConfig { path, target } in &config.redirect {
            if let Err(e) = write_redirect_page(overrides.format, path, target) {
                eprintln!("skipping redirect {}: {}", path.display(), e);
            }
        }
    }

    config.id = Some(id);
    config.url = url;
    config.inline = overrides.inline.unwrap_or(config.inline);

    Some(config)
}
