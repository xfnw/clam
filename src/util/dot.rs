use crate::{helpers::org_urls, util::map_org};
use git2::{Commit, Repository};
use orgize::Org;
use std::{
    fmt::{self, Write},
    path::Path,
};
use url::Url;

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

fn eat_file_index(url: &mut Url) {
    let Some("index.org") = url.path_segments().and_then(|mut i| i.next_back()) else {
        return;
    };
    let Ok(mut segments) = url.path_segments_mut() else {
        return;
    };
    segments.pop().push("");
}

pub fn print_dot(repo: &Repository, commit: &Commit) {
    println!(
        r"digraph L {{
node [color=gray];
rankdir=LR;"
    );

    map_org(repo, commit, |name, blob| {
        let fstr = std::str::from_utf8(blob.content()).unwrap();
        let res = Org::parse(fstr);
        let mut base = Url::from_file_path(Path::new("/").join(name))
            .expect("current path should fit in a file url");
        eat_file_index(&mut base);
        let from = DotEscape(base.as_str());
        org_urls(&res, &base, |mut url| {
            match url.scheme() {
                "abbr" => return,
                "file" => {}
                _ => (),
            }
            url.set_fragment(None);
            eat_file_index(&mut url);
            println!("{from} -> {};", DotEscape(url.as_str()));
        });
        println!("{from} [color=black];");
    })
    .unwrap();

    println!("}}");
}
