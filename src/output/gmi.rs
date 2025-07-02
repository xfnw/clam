use crate::{
    config::ClamConfig,
    git::{HistMap, HistMeta},
    helpers::{org_links, URL_PATH_UNSAFE},
    output::{get_keywords, infer_title, PageMetadata, Pages},
    Error,
};
use chrono::{DateTime, Datelike};
use orgize::{
    export::{Container, Event, TraversalContext, Traverser},
    ParseConfig,
};
use percent_encoding::utf8_percent_encode;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(boilerplate::Boilerplate)]
struct PageGmi<'a> {
    title: &'a str,
    body: &'a str,
    numdir: usize,
    notice: Option<&'static str>,
    metadata: Option<&'a PageMetadata<'a>>,
}

#[derive(Default)]
struct GmiExport {
    output: String,
}

impl GmiExport {
    fn push_str(&mut self, s: impl AsRef<str>) {
        self.output += s.as_ref();
    }
    fn push_join(&mut self, s: impl AsRef<str>) {
        let mut splitted = s.as_ref().split('\n');
        if let Some(l) = splitted.next() {
            self.output += l;
            for l in splitted {
                self.output.push(' ');
                self.output += l.trim_start();
            }
        }
    }
    fn finish(self) -> String {
        self.output
    }
}

impl Traverser for GmiExport {
    fn event(&mut self, event: Event, ctx: &mut TraversalContext) {
        match event {
            Event::Enter(Container::Keyword(_)) => ctx.skip(),
            Event::Leave(Container::Paragraph(_)) => {
                // TODO: output links here
                self.push_str("\n\n");
            }
            Event::Text(text) => self.push_join(text),
            Event::Cookie(cookie) => {
                self.push_str(cookie.raw());
            }
            Event::LineBreak(_) => self.output.push('\n'),
            _ => (),
        }
    }
}

pub fn generate_page(
    dir: &str,
    name: &str,
    file: &[u8],
    org_cfg: &ParseConfig,
    pages: &mut Pages,
    links: &mut HashMap<PathBuf, Vec<Rc<PathBuf>>>,
) -> Result<(), Error> {
    let mut full_path: PathBuf = format!("{dir}{name}").into();
    if Some("org") == full_path.extension().and_then(OsStr::to_str) {
        let fstr = std::str::from_utf8(file).map_err(Error::NonUTF8Org)?;
        let res = org_cfg.clone().parse(fstr);

        let title = res.title().unwrap_or_else(|| infer_title(&full_path));

        let old_path = full_path.clone();
        full_path.set_extension("gmi");

        let mypath = Rc::new(full_path.clone());
        org_links(&res, &full_path, |mut l| {
            l.set_extension("gmi");

            if let Some(e) = links.get_mut(&l) {
                e.push(mypath.clone());
            } else {
                links.insert(l, vec![mypath.clone()]);
            }
        });

        let keywords = get_keywords(&res);
        let mut gmi_export = GmiExport::default();
        res.traverse(&mut gmi_export);
        let gmi = gmi_export.finish();

        pages.insert(full_path, (title, old_path, keywords, gmi));
    } else {
        let mut f = File::create(full_path).map_err(Error::File)?;
        f.write_all(file).map_err(Error::File)?;
    }
    Ok(())
}

pub fn write_org_page(
    pages: &Pages,
    hist: &HistMap,
    links: &HashMap<PathBuf, Vec<Rc<PathBuf>>>,
    short_id: &str,
    _config: Option<&ClamConfig>,
) -> Result<(), Error> {
    let year_ago = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(Error::Clock)?
        .as_secs()
        - 365 * 24 * 60 * 60;
    let year_ago: i64 = year_ago.try_into().map_err(|_| Error::TimeOverflow)?;

    for (new_path, (title, old_path, keywords, html)) in pages {
        let HistMeta {
            create_time,
            modify_time,
            creator,
            contributors,
            ..
        } = hist.get(old_path).ok_or(Error::MissingHist)?;

        let author = keywords.author.as_deref().unwrap_or(creator);
        let year = if let Some(Ok(year)) = keywords.year.as_ref().map(|k| k.parse()) {
            year
        } else {
            DateTime::from_timestamp(create_time.seconds(), 0)
                .ok_or(Error::BadCreateTime)?
                .naive_utc()
                .year()
        };

        let numdir = old_path.iter().count();

        let notice = if modify_time.seconds() - year_ago < 0 {
            Some("this page was last updated over a year ago. facts and circumstances may have changed since.")
        } else {
            None
        };

        let incoming: Option<HashSet<_>> = links.get(new_path).map(|l| l.iter().collect());
        let incoming: Option<Vec<_>> = incoming.map(|l| {
            l.iter()
                .map(|b| {
                    (
                        b.to_str().unwrap(),
                        pages.get(b.as_ref()).unwrap().0.as_ref(),
                    )
                })
                .collect()
        });

        let contributors = contributors.len() - usize::from(contributors.contains(author));

        let meta = PageMetadata {
            author,
            commit: short_id,
            modified: DateTime::from_timestamp(modify_time.seconds(), 0)
                .ok_or(Error::BadModifyTime)?
                .naive_utc(),
            year,
            incoming: incoming.as_deref(),
            footer: None,
            contributors,
        };

        let template = PageGmi {
            title,
            body: html,
            numdir,
            notice,
            metadata: Some(&meta),
        };

        let mut f = File::create(new_path).map_err(Error::File)?;
        f.write_all(&template.to_string().into_bytes())
            .map_err(Error::File)?;
    }
    Ok(())
}

pub fn write_redirect_page(_path: &Path, _target: &str) -> Result<(), Error> {
    todo!()
}
