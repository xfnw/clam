use crate::git::{CreateMap, ModifyMap};
use chrono::{DateTime, Datelike, NaiveDateTime};
use html_escaper::{Escape, Trusted};
use orgize::{
    ast::{Headline, TodoType},
    export::{Container, Event, HtmlEscape, HtmlExport, TraversalContext, Traverser},
    ParseConfig,
};
use rowan::ast::AstNode;
use slugify::slugify;
use std::{cmp::min, collections::BTreeMap, error::Error, fs, io::Write, path::PathBuf};

#[derive(boilerplate::Boilerplate)]
struct PageHtml<'a> {
    title: String,
    body: String,
    commit: &'a str,
    author: &'a str,
    created: NaiveDateTime,
    modified: NaiveDateTime,
    numdir: usize,
    old_page: bool,
}

#[derive(Default)]
pub struct Handler {
    pub exp: HtmlExport,
    pub numdir: usize,
}

impl Traverser for Handler {
    fn event(&mut self, event: Event, ctx: &mut TraversalContext) {
        match event {
            Event::Enter(Container::Headline(headline)) => {
                let lvl = headline.level();
                let lvl = 1 + min(lvl, 5);

                let id = generate_headline_id(&headline);

                self.exp.push_str(format!("<h{} id=\"{}\">", lvl, id));
                self.output_headline_todo(&headline);
                for e in headline.title() {
                    self.element(e, ctx);
                }
                self.exp.push_str(format!(
                    r##" <a class=see-focus href="#{}" aria-label="permalink to section">¶</a></h{}>"##,
                    id, lvl
                ));
            }
            Event::Enter(Container::Link(link)) => {
                let path = link.path();
                let path = path.trim_start_matches("file:");
                let path = if let Some(p) = path.strip_prefix('*') {
                    let mut p = slugify!(p);
                    p.insert(0, '#');
                    p
                } else if path.starts_with("//") || path.contains("://") {
                    path.to_string()
                } else if let Some(p) = path.strip_suffix(".org") {
                    let mut p = p.to_string();
                    p.push_str(".html");
                    p
                } else if path.contains(".org#") {
                    path.replace(".org#", ".html#")
                } else {
                    path.to_string()
                };

                if link.is_image() {
                    if let Some(Some(caption)) = link.caption().map(|c| c.value()) {
                        self.exp.push_str(format!(
                            r#"<img src="{}" alt="{}">"#,
                            HtmlEscape(&path),
                            HtmlEscape(caption.trim())
                        ));
                    } else {
                        self.exp
                            .push_str(format!("<img src=\"{}\">", HtmlEscape(&path)));
                    }
                    return ctx.skip();
                }

                self.exp
                    .push_str(format!("<a href=\"{}\">", HtmlEscape(&path)));

                if !link.has_description() {
                    self.exp.push_str(format!("{}</a>", HtmlEscape(&path)));
                    ctx.skip();
                }
            }
            Event::Enter(Container::SpecialBlock(block)) => {
                if let Some(mut par) = block
                    .syntax()
                    .first_child()
                    .map(|n| n.children_with_tokens().filter_map(|t| t.into_token()))
                {
                    if let Some(name) = par.nth(1) {
                        let name = name.text();

                        self.exp
                            .push_str(format!("<div class=\"{}\">", HtmlEscape(&name)));

                        if name.eq_ignore_ascii_case("chat") {
                            if let Some(usr) = par.next() {
                                let usr = usr.text().trim();
                                if !usr.is_empty() {
                                    let (person, expression) =
                                        usr.rsplit_once('/').unwrap_or((usr, usr));
                                    self.exp.push_str("<img class=chat-head width=64 src=\"");
                                    for _ in 1..self.numdir {
                                        self.exp.push_str("../");
                                    }
                                    self.exp.push_str(format!(
                                        r#"faces/{}.png" alt="{} is {}"><div class=chat-text><span class=chat-nick role=figure aria-label="{1} says">&lt;{1}&gt;</span> "#,
                                        slugify!(usr), HtmlEscape(person), HtmlEscape(expression)
                                    ));

                                    self.output_block_children(block, ctx);

                                    self.exp.push_str("</div></div>");

                                    return ctx.skip();
                                }
                            }
                        }

                        self.output_block_children(block, ctx);

                        self.exp.push_str("</div>");
                    }
                }
                ctx.skip();
            }
            Event::Enter(Container::ListItem(ref item)) => {
                // pretend indeterminate checkboxes do not exist and
                // shove the state into a bool. html does not have a
                // good way to create indeterminate checkboxes unless
                // using javascript >:(
                let checked = item.checkbox().map(|s| s.as_bytes() == b"X");

                // orgize's ListItem implementation uses weird private fields,
                // easier to just reuse it (though bye bye item reference)
                self.exp.event(event, ctx);

                if let Some(state) = checked {
                    self.exp.push_str(if state {
                        "<input type=checkbox disabled checked /> "
                    } else {
                        "<input type=checkbox disabled /> "
                    })
                }
            }
            Event::Enter(Container::Keyword(keyword)) => {
                if keyword.key().eq_ignore_ascii_case("TOC") {
                    self.exp
                        .push_str("<details><summary>table of contents</summary>");

                    if let Some(Some(parent)) = keyword.syntax().parent().map(|p| p.parent()) {
                        let mut depth = 0;
                        for descendant in parent.descendants() {
                            if let Some(headline) = Headline::cast(descendant) {
                                let level = headline.level();
                                while depth < level {
                                    self.exp.push_str("<ul>");
                                    depth += 1;
                                }
                                while depth > level {
                                    self.exp.push_str("</ul>");
                                    depth -= 1;
                                }

                                self.exp.push_str(format!(
                                    "<li><a href=\"#{}\">",
                                    generate_headline_id(&headline)
                                ));
                                self.output_headline_todo(&headline);
                                for e in headline.title() {
                                    self.element(e, ctx);
                                }
                                self.exp.push_str("</a></li>");
                            }
                        }
                        while depth > 0 {
                            self.exp.push_str("</ul>");
                            depth -= 1;
                        }
                    }

                    self.exp.push_str("</details>");
                }
                ctx.skip();
            }
            Event::Enter(Container::Subscript(sub)) => {
                self.exp.push_str(HtmlEscape(sub.raw()).to_string());
                ctx.skip();
            }
            Event::Cookie(cookie) => {
                self.exp.push_str(HtmlEscape(cookie.raw()).to_string());
            }
            _ => self.exp.event(event, ctx),
        };
    }
}

impl Handler {
    /// output children while stripping off some exterior formatting
    fn output_block_children(
        &mut self,
        block: orgize::ast::SpecialBlock,
        ctx: &mut TraversalContext,
    ) {
        for child in block.syntax().children() {
            for sub in child.children() {
                for e in sub.children_with_tokens() {
                    self.element(e, ctx);
                }
            }
        }
    }

    fn output_headline_todo(&mut self, headline: &Headline) {
        if let Some(keyword) = headline.todo_keyword() {
            self.exp.push_str(match headline.todo_type() {
                Some(TodoType::Todo) => {
                    format!("<span class=todo>{}</span> ", HtmlEscape(keyword.as_ref()))
                }
                Some(TodoType::Done) => {
                    format!("<span class=done>{}</span> ", HtmlEscape(keyword.as_ref()))
                }
                None => unreachable!(),
            });
        }
    }
}

fn generate_headline_id(headline: &Headline) -> String {
    let txt: String = headline.title().map(|t| t.to_string()).collect();

    if let Some(Some(cid)) = headline.properties().map(|p| p.get("CUSTOM_ID")) {
        HtmlEscape(cid).to_string()
    } else {
        slugify!(&txt)
    }
}

pub fn generate_page(
    dir: &str,
    name: &str,
    file: &[u8],
    org_cfg: &ParseConfig,
    ctime: &CreateMap,
    mtime: &ModifyMap,
    year_ago: i64,
    short_id: &str,
    titles: &mut BTreeMap<PathBuf, (String, PathBuf)>,
) -> Result<(), Box<dyn Error>> {
    let mut full_path: PathBuf = format!("{}{}", dir, name).into();
    let pcontent: Option<Vec<u8>> = match full_path.extension().and_then(std::ffi::OsStr::to_str) {
        Some("org") => {
            let fstr = std::str::from_utf8(file)?;
            let res = org_cfg.clone().parse(fstr);

            let title = res
                .document()
                .title()
                .unwrap_or_else(|| "untitled".to_string());

            let (created, author) = ctime.get(&full_path).ok_or("missing creation time")?;
            let modified = mtime.get(&full_path).ok_or("missing modification time")?.0;

            let numdir = full_path.iter().count();

            let mut html_export = Handler {
                numdir,
                ..Default::default()
            };
            res.traverse(&mut html_export);

            let old_page = modified.seconds() - year_ago < 0;

            let template = PageHtml {
                title: title.clone(),
                body: html_export.exp.finish(),
                commit: short_id,
                author,
                created: DateTime::from_timestamp(created.seconds(), 0)
                    .ok_or("broken creation date")?
                    .naive_utc(),
                modified: DateTime::from_timestamp(modified.seconds(), 0)
                    .ok_or("broken modification date")?
                    .naive_utc(),
                numdir,
                old_page,
            };

            let old_path = full_path.clone();
            full_path.set_extension("html");
            titles.insert(full_path.clone(), (title, old_path));

            Some(template.to_string().into_bytes())
        }
        _ => None,
    };
    let content = match &pcontent {
        Some(c) => c,
        None => file,
    };
    let mut f = fs::File::create(full_path)?;
    f.write_all(content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::html::*;
    use orgize::Org;

    #[test]
    fn generate_html() {
        let res = Org::parse(
            r#"#+TITLE: you should not see this
* meow
#+begin_chat fox
AAAA
#+end_chat

this__has__under_scores yip_{yap yop}

[[*finish writing this test][i am a heading link]]
[[hmm/example.org/test.org][should link to .html]]
[[hmm/example.org/test.org#something][should also link to .html]]
[[hmm/example.org/][im a directory!]]
[[https://example.org][webbed sight]]

#+CAPTION: the libera.chat logo, but with the mountain replaced with a cat
[[https://cheapiesystems.com/media/images/libera-cat.png]]

** TODO wash the fox
:PROPERTIES:
:CUSTOM_ID: foxwash-time
:END:

#+begin_chat fox/stimky
AAAA even more
#+end_chat

** DONE finish writing this test"#,
        );
        let mut exp = Handler::default();
        res.traverse(&mut exp);
        assert_eq!(
            exp.exp.finish(),
            r##"<main><section></section><h2 id="meow">meow <a class=see-focus href="#meow" aria-label="permalink to section">¶</a></h2><section><div class="chat"><img class=chat-head width=64 src="faces/fox.png" alt="fox is fox"><div class=chat-text><span class=chat-nick role=figure aria-label="fox says">&lt;fox&gt;</span> AAAA
</div></div><p>this__has__under_scores yip_{yap yop}
</p><p><a href="#finish-writing-this-test">i am a heading link</a>
<a href="hmm/example.org/test.html">should link to .html</a>
<a href="hmm/example.org/test.html#something">should also link to .html</a>
<a href="hmm/example.org/">im a directory!</a>
<a href="https://example.org">webbed sight</a>
</p><p><img src="https://cheapiesystems.com/media/images/libera-cat.png" alt="the libera.chat logo, but with the mountain replaced with a cat">
</p></section><h3 id="foxwash-time"><span class=todo>TODO</span> wash the fox <a class=see-focus href="#foxwash-time" aria-label="permalink to section">¶</a></h3><section><p></p><div class="chat"><img class=chat-head width=64 src="faces/fox-stimky.png" alt="fox is stimky"><div class=chat-text><span class=chat-nick role=figure aria-label="fox says">&lt;fox&gt;</span> AAAA even more
</div></div></section><h3 id="finish-writing-this-test"><span class=done>DONE</span> finish writing this test <a class=see-focus href="#finish-writing-this-test" aria-label="permalink to section">¶</a></h3></main>"##
        );
    }
}
