use crate::{
    config::ClamConfig,
    git::HistMap,
    helpers::{org_links, URL_PATH_UNSAFE},
    output::{get_keywords, infer_title, PageMetadata, Pages},
    Error,
};
use orgize::{
    export::{Container, Event, TraversalContext, Traverser},
    ParseConfig,
};
use percent_encoding::utf8_percent_encode;
use std::{
    collections::HashMap,
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
    fn finish(self) -> String {
        self.output
    }
}

impl Traverser for GmiExport {
    fn event(&mut self, event: Event, ctx: &mut TraversalContext) {
        match event {
            Event::Enter(Container::Keyword(_)) => ctx.skip(),
            // TODO: combine paragraphs into one line
            Event::Text(text) => self.push_str(text),
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
    _pages: &Pages,
    _hist: &HistMap,
    _links: &HashMap<PathBuf, Vec<Rc<PathBuf>>>,
    _short_id: &str,
    _config: Option<&ClamConfig>,
) -> Result<(), Error> {
    todo!()
}

pub fn write_redirect_page(_path: &Path, _target: &str) -> Result<(), Error> {
    todo!()
}
