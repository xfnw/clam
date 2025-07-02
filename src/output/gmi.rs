use crate::{
    config::ClamConfig,
    git::HistMap,
    helpers::URL_PATH_UNSAFE,
    output::{PageMetadata, Pages},
    Error,
};
use orgize::{
    export::{Container, Event, TraversalContext, Traverser},
    ParseConfig,
};
use percent_encoding::utf8_percent_encode;
use std::{
    collections::HashMap,
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
    _dir: &str,
    _name: &str,
    _file: &[u8],
    _org_cfg: &ParseConfig,
    _pages: &mut Pages,
    _links: &mut HashMap<PathBuf, Vec<Rc<PathBuf>>>,
) -> Result<(), Error> {
    todo!()
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
