use crate::{config::ClamConfig, git::HistMap, output::Pages, Error};
use orgize::ParseConfig;
use std::{collections::HashMap, path::PathBuf, rc::Rc};

pub fn write_org_page(
    _pages: &Pages,
    _hist: &HistMap,
    _links: &HashMap<PathBuf, Vec<Rc<PathBuf>>>,
    _short_id: &str,
    _config: Option<&ClamConfig>,
) -> Result<(), Error> {
    todo!()
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
