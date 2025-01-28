use super::{find_links, map_org};
use git2::{Commit, Repository};
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Write},
    path::PathBuf,
    rc::Rc,
};

struct DotEscape<'a>(&'a str);

impl fmt::Display for DotEscape<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char('"')?;
        for c in self.0.chars() {
            match c {
                '\\' => f.write_str("\\\\")?,
                '"' => f.write_str("\\\"")?,
                '\n' => f.write_str("\\n")?,
                '\r' => f.write_str("\\r")?,
                a => f.write_char(a)?,
            }
        }
        f.write_char('"')
    }
}

pub fn print_dot(repo: &Repository, commit: &Commit) {
    let mut pages = HashSet::new();
    let mut links: HashMap<PathBuf, Vec<Rc<PathBuf>>> = HashMap::new();

    map_org(repo, commit, |name, blob| {
        let name = pages.get(&name).cloned().unwrap_or_else(|| {
            let name = Rc::new(name);
            pages.insert(name.clone());
            name
        });
        find_links(&name, &blob, |l| match links.get_mut(&l) {
            Some(v) => {
                v.push(name.clone());
            }
            None => {
                links.insert(l, vec![name.clone()]);
            }
        });
    })
    .unwrap();

    println!(
        r"digraph L {{
rankdir=LR;"
    );

    for page in pages {
        let pname = DotEscape(page.to_str().unwrap());
        println!("{pname};");
        if let Some(inlinks) = links.get(page.as_ref()) {
            for link in inlinks {
                println!("{} -> {};", DotEscape(link.to_str().unwrap()), pname);
            }
        }
    }

    println!("}}");
}
