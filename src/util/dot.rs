use crate::{RepoArgs, helpers::org_urls, util::map_org};
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

pub fn print_dot(repo: &Repository, commit: &Commit, args: &RepoArgs) {
    let root = if let Some(url) = &args.url {
        Url::parse(url).expect("you should pass a valid url to the url option")
    } else {
        Url::from_file_path(Path::new("/")).unwrap()
    };

    println!(
        r"digraph L {{
node [color=gray];
rankdir=LR;"
    );

    map_org(repo, commit, |mut name, blob| {
        let Ok(fstr) = str::from_utf8(blob.content()) else {
            return;
        };
        if args.url.is_some() {
            name.set_extension(args.format.to_ext());
        }
        let nstr = name.to_str().unwrap();
        let res = Org::parse(fstr);
        let mut base = root
            .join(nstr)
            .expect("current path should parse as a url path");
        eat_file_index(&mut base);
        let from = DotEscape(base.as_str());
        org_urls(&res, &base, |mut url| {
            if url.scheme() == "abbr" {
                return;
            }
            url.set_fragment(None);
            eat_file_index(&mut url);
            if args.url.is_some()
                && url.as_str().starts_with(root.as_str())
                && let Some((pre, ext)) = url.path().rsplit_once('.')
                && ext.eq_ignore_ascii_case("org")
            {
                url.set_path(&format!("{pre}.{}", args.format.to_ext()));
            }
            println!("{from} -> {};", DotEscape(url.as_str()));
        });
        print!("{from} [color=black");
        if args.url.is_some() {
            print!(";URL={}", DotEscape(base.as_str()));
        }
        println!("];");
    })
    .unwrap();

    println!("}}");
}
