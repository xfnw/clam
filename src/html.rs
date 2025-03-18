use crate::{
    config::ClamConfig,
    git::{CreateMap, ModifyMap},
    helpers::org_links,
    Error, STYLESHEET_STR,
};
use chrono::{DateTime, Datelike, NaiveDateTime};
use html_escaper::{Escape, Trusted};
use indexmap::IndexMap;
use orgize::{
    ast::{Headline, TodoType, Token},
    export::{Container, Event, HtmlEscape, HtmlExport, TraversalContext, Traverser},
    rowan::{ast::AstNode, NodeOrToken},
    Org, ParseConfig, SyntaxKind, SyntaxNode, SyntaxToken,
};
use slugify::slugify;
use std::{
    cmp::min,
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    rc::Rc,
};

#[derive(boilerplate::Boilerplate, Default)]
pub struct PageHtml<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub lang: &'a str,
    pub numdir: usize,
    pub notice: Option<&'static str>,
    pub header: Option<&'a str>,
    pub metadata: Option<&'a PageMetadata<'a>>,
    pub nav: bool,
    pub inline: bool,
}

pub struct PageMetadata<'a> {
    pub author: &'a str,
    pub commit: &'a str,
    pub modified: NaiveDateTime,
    pub year: i32,
    pub incoming: Option<&'a [(&'a str, &'a str)]>,
    pub footer: Option<&'a str>,
}

#[derive(Default)]
pub struct PageKeywords {
    pub author: Option<String>,
    pub language: Option<String>,
    pub year: Option<String>,
}

pub type TokenList = Vec<NodeOrToken<SyntaxNode, SyntaxToken>>;
pub type Pages = HashMap<PathBuf, (String, PathBuf, PageKeywords, String)>;

#[derive(Default)]
pub struct Handler {
    pub exp: HtmlExport,
    pub numdir: usize,
    pub feet: IndexMap<String, (Option<TokenList>, i32)>,
}

impl Traverser for Handler {
    fn event(&mut self, event: Event, ctx: &mut TraversalContext) {
        match event {
            Event::Enter(Container::Headline(headline)) => {
                let lvl = headline.level();
                let lvl = 1 + min(lvl, 5);

                let id = generate_headline_id(&headline);

                self.exp
                    .push_str(format!("<h{lvl} tabindex=-1 id=\"{id}\">"));
                self.output_headline_todo(&headline);
                for e in headline.title() {
                    self.element(e, ctx);
                }
                self.exp.push_str(format!(
                    r##" <a class=see-focus href="#{id}" aria-label="permalink to section">¶</a></h{lvl}>"##,
                ));
            }
            Event::Enter(Container::Link(link)) => {
                let path = link.path();

                if let Some(meaning) = path.strip_prefix("abbr:") {
                    self.exp
                        .push_str(format!("<abbr title=\"{}\">", HtmlEscape(meaning)));
                    return;
                }

                let path = mangle_link(&path);

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
            Event::Leave(Container::Link(link)) => {
                if link.path().starts_with("abbr:") {
                    self.exp.push_str("</abbr>");
                    return;
                }
                self.exp.push_str("</a>");
            }
            Event::Enter(Container::SpecialBlock(block)) => {
                if let Some(mut par) = block
                    .syntax()
                    .first_child()
                    .map(|n| n.children_with_tokens().filter_map(NodeOrToken::into_token))
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
                                        r#"faces/{}.png" alt="{} is {} says"><div class=chat-text><span class=chat-nick aria-hidden=true>&lt;{1}&gt;</span> "#,
                                        slugify!(usr), HtmlEscape(person), HtmlEscape(expression)
                                    ));

                                    self.output_block_children(&block, ctx);

                                    self.exp.push_str("</div></div>");

                                    return ctx.skip();
                                }
                            }
                        }

                        self.output_block_children(&block, ctx);

                        self.exp.push_str("</div>");
                    }
                }
                ctx.skip();
            }
            Event::Enter(Container::VerseBlock(_)) => self.exp.push_str("<pre class=verse>"),
            Event::Enter(Container::FixedWidth(_)) => self.exp.push_str("<pre class=example>"),
            Event::Leave(Container::VerseBlock(_) | Container::FixedWidth(_)) => {
                self.exp.push_str("</pre>");
            }
            Event::Enter(Container::ExportBlock(block)) => {
                if Some(true) == block.ty().map(|b| b.eq_ignore_ascii_case("html")) {
                    self.exp.push_str(block.value());
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
                        "<input type=checkbox disabled checked> "
                    } else {
                        "<input type=checkbox disabled> "
                    });
                }
            }
            Event::Enter(Container::Keyword(keyword)) => {
                if !keyword.key().eq_ignore_ascii_case("TOC") {
                    ctx.skip();
                    return;
                }

                let value = keyword.value();
                let mut value = value.split_ascii_whitespace();
                match value.next() {
                    Some("headlines") | None => (),
                    Some(o) => {
                        eprintln!("TOC type {o} not supported");
                        ctx.skip();
                        return;
                    }
                }
                let limit: usize = if let Some(Ok(val)) = value.next().map(str::parse) {
                    val
                } else {
                    0
                };

                self.exp
                    .push_str("<details class=toc><summary>table of contents</summary>");

                if let Some(Some(parent)) = keyword.syntax().parent().map(|p| p.parent()) {
                    let mut depth = 0;
                    for descendant in parent.descendants() {
                        if let Some(headline) = Headline::cast(descendant) {
                            let level = headline.level();
                            if limit != 0 && level > limit {
                                continue;
                            }
                            if depth == level {
                                self.exp.push_str("</li>");
                            } else {
                                while depth < level {
                                    self.exp.push_str("<ul>");
                                    depth += 1;
                                }
                                while depth > level {
                                    self.exp.push_str("</li></ul>");
                                    depth -= 1;
                                }
                            }

                            self.exp.push_str(format!(
                                "<li><a href=\"#{}\">",
                                generate_headline_id(&headline)
                            ));
                            self.output_headline_todo(&headline);
                            for e in headline.title() {
                                self.element(e, ctx);
                            }
                            self.exp.push_str("</a>");
                        }
                    }
                    while depth > 0 {
                        self.exp.push_str("</li></ul>");
                        depth -= 1;
                    }
                }

                self.exp.push_str("</details>");
                ctx.skip();
            }
            // FIXME: this is cursed, replace with fancy new footnote stuff once merged into orgize
            // https://github.com/PoiScript/orgize/pull/84
            Event::Enter(Container::FnDef(foot)) => {
                let mut children = foot.syntax().children_with_tokens().skip(3);
                if let Some(Some(name)) = children.next().map(NodeOrToken::into_token) {
                    let name = name.text();
                    let def = children
                        .next()
                        .is_some_and(|c| c.kind() == SyntaxKind::R_BRACKET);
                    if def {
                        if let Some(note) = self.feet.get_mut(name) {
                            note.0 = Some(children.collect());
                        } else {
                            self.feet
                                .insert(name.to_string(), (Some(children.collect()), -1));
                        }
                    }
                }
                ctx.skip();
            }
            Event::Enter(Container::FnRef(foot)) => {
                let mut children = foot.syntax().children_with_tokens().skip(3);
                if let Some(Some(name)) = children.next().map(NodeOrToken::into_token) {
                    let name = name.text();
                    let def = children
                        .next()
                        .is_some_and(|c| c.kind() == SyntaxKind::COLON);
                    let (fnum, rnum) = if let Some(note) = self.feet.get_full_mut(name) {
                        note.2 .1 += 1;
                        if def {
                            note.2 .0 = Some(children.collect());
                        }
                        (note.0, note.2 .1)
                    } else {
                        let n = self.feet.len();
                        self.feet.insert(
                            if name.is_empty() {
                                format!(":{n}") // footnote name cannot contain colons
                            } else {
                                name.to_string()
                            },
                            (if def { Some(children.collect()) } else { None }, 0),
                        );
                        (n, 0)
                    };
                    let fnum = fnum + 1;
                    self.exp.push_str(format!(
                        r##"<sup><a id="fnr.{fnum}.{rnum}" href="#fn.{fnum}" role=doc-noteref>[{fnum}]</a></sup>"##
                    ));
                }
                ctx.skip();
            }
            Event::Leave(Container::Document(_)) => {
                if !self.feet.is_empty() {
                    self.exp.push_str("<section role=doc-endnotes aria-labelledby=clam.footnotes><h2 id=clam.footnotes>footnotes</h2><ol>");
                    for (n, (_, (elem, refs))) in self.feet.iter().enumerate() {
                        let n = n + 1;
                        self.exp.push_str(format!("<li id=\"fn.{n}\">"));
                        if let Some(elem) = elem {
                            for e in elem {
                                // bad workaround for self.element needing &mut self
                                self.exp.element(e.clone(), ctx);
                            }
                        }
                        self.exp.push_str("\n");
                        for r in 0..=*refs {
                            self.exp.push_str(format!(
                                "<a href=\"#fnr.{n}.{r}\" role=doc-backlink>↩</a>\n"
                            ));
                        }
                        self.exp.push_str("</li>");
                    }
                    self.exp.push_str("</ol></section>");
                }

                self.exp.push_str("</main>");
            }
            Event::Cookie(cookie) => {
                self.exp.push_str(HtmlEscape(cookie.raw()).to_string());
            }
            Event::LineBreak(_) => self.exp.push_str("<br>"),
            _ => self.exp.event(event, ctx),
        };
    }
}

impl Handler {
    /// output children while stripping off some exterior formatting
    fn output_block_children(
        &mut self,
        block: &orgize::ast::SpecialBlock,
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

fn mangle_link(path: &Token) -> String {
    let path = path.strip_prefix("file:").unwrap_or(path);
    if let Some(p) = path.strip_prefix('*') {
        let mut p = slugify!(p);
        p.insert(0, '#');
        return p;
    }
    if path.starts_with("//") || path.contains("://") {
        return path.to_string();
    }
    if let Some(p) = path.strip_suffix(".org") {
        let mut p = p.to_string();
        p.push_str(".html");
        return p;
    }
    path.replace(".org#", ".html#")
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
    if Some("org") == full_path.extension().and_then(std::ffi::OsStr::to_str) {
        let fstr = std::str::from_utf8(file).map_err(|_| Error::NonUTF8Path)?;
        let res = org_cfg.clone().parse(fstr);

        let title = res.title().unwrap_or_else(|| "untitled".to_string());

        let old_path = full_path.clone();
        full_path.set_extension("html");

        let mypath = Rc::new(full_path.clone());
        org_links(&res, &full_path, |mut l| {
            l.set_extension("html");

            if let Some(e) = links.get_mut(&l) {
                e.push(mypath.clone());
            } else {
                links.insert(l, vec![mypath.clone()]);
            }
        });

        let keywords = get_keywords(&res);
        let mut html_export = Handler {
            numdir: old_path.iter().count(),
            ..Default::default()
        };
        res.traverse(&mut html_export);
        let html = html_export.exp.finish();

        pages.insert(full_path, (title, old_path, keywords, html));
    } else {
        let mut f = fs::File::create(full_path).map_err(Error::File)?;
        f.write_all(file).map_err(Error::File)?;
    }
    Ok(())
}

pub fn write_org_page(
    pages: &Pages,
    ctime: &CreateMap,
    mtime: &ModifyMap,
    links: &HashMap<PathBuf, Vec<Rc<PathBuf>>>,
    short_id: &str,
    config: Option<&ClamConfig>,
) -> Result<(), Error> {
    let year_ago = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(Error::Clock)?
        .as_secs()
        - 365 * 24 * 60 * 60;
    let year_ago: i64 = year_ago.try_into().map_err(|_| Error::TimeOverflow)?;

    let (header, footer, nav, inline) = config.map_or((None, None, false, false), |conf| {
        (
            conf.extra_header.as_deref(),
            conf.extra_footer.as_deref(),
            conf.show_navigation,
            conf.inline,
        )
    });

    for (new_path, (title, old_path, keywords, html)) in pages {
        let (created, author) = ctime.get(old_path).ok_or(Error::NoCreateTime)?;
        let modified = mtime.get(old_path).ok_or(Error::NoModifyTime)?.0;

        let author = keywords.author.as_deref().unwrap_or(author);
        let lang = keywords.language.as_deref().unwrap_or("en");
        let year = if let Some(Ok(year)) = keywords.year.as_ref().map(|k| k.parse()) {
            year
        } else {
            DateTime::from_timestamp(created.seconds(), 0)
                .ok_or(Error::BadCreateTime)?
                .naive_utc()
                .year()
        };

        let numdir = old_path.iter().count();

        let notice = if modified.seconds() - year_ago < 0 {
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

        let meta = PageMetadata {
            author,
            commit: short_id,
            modified: DateTime::from_timestamp(modified.seconds(), 0)
                .ok_or(Error::BadModifyTime)?
                .naive_utc(),
            year,
            incoming: incoming.as_deref(),
            footer,
        };

        let template = PageHtml {
            title,
            body: html,
            lang,
            numdir,
            notice,
            header,
            nav,
            inline,
            metadata: Some(&meta),
        };

        let mut f = fs::File::create(new_path).map_err(Error::File)?;
        f.write_all(&template.to_string().into_bytes())
            .map_err(Error::File)?;
    }
    Ok(())
}

pub fn write_redirect_page(path: &Path, target: &str) -> Result<(), Error> {
    if path.components().any(|s| {
        matches!(
            s,
            Component::RootDir | Component::ParentDir | Component::CurDir
        )
    }) {
        return Err(Error::UnsafePath);
    }
    let body = format!(
        "this page has been <a href=\"{}\">moved here</a>.",
        HtmlEscape(&target)
    );
    let refresh = format!(
        "<meta http-equiv=refresh content=\"0;URL='{}'\">",
        HtmlEscape(&target)
    );
    let numdir = path.iter().count();
    let template = PageHtml {
        title: "redirecting...",
        body: &body,
        header: Some(&refresh),
        lang: "en",
        numdir,
        ..Default::default()
    };
    let mut f = fs::File::create(path).map_err(Error::File)?;
    f.write_all(&template.to_string().into_bytes())
        .map_err(Error::File)
}

pub fn get_keywords(res: &Org) -> PageKeywords {
    macro_rules! match_keywords {
        ($k:ident, $kw:ident, ($($key:ident),*)) => {
            match $k.key().to_ascii_lowercase().as_ref() {
                $(stringify!($key) => {
                    if $kw.$key.is_none() {
                        $kw.$key = Some($k.value().trim().to_string());
                    }
                })*
                _ => {}
            }
        }
    }
    let mut keywords = PageKeywords::default();
    for k in res.keywords() {
        match_keywords!(k, keywords, (author, language, year));
    }
    keywords
}

#[cfg(test)]
mod tests {
    use crate::html::*;
    use orgize::Org;

    #[test]
    fn snapshot_html() {
        let res = Org::parse(
            r#"#+TITLE: you should not see this
* meow
#+begin_chat fox
AAAA
#+end_chat

i have a footnote[fn:1:beep /boop/][fn:2]

[fn:2] and *another* footnote

[[*finish writing this test][i am a heading link]]
[[file:hmm/example.org/test.org][should link to .html]]
[[hmm/example.org/test.org#something][should also link to .html]]
[[hmm/example.org/][im a directory!]]
[[abbr:Yelling In Furry Form][YIFF]] is an acronym
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
            r##"<main><section></section><h2 tabindex=-1 id="meow">meow <a class=see-focus href="#meow" aria-label="permalink to section">¶</a></h2><section><div class="chat"><img class=chat-head width=64 src="faces/fox.png" alt="fox is fox says"><div class=chat-text><span class=chat-nick aria-hidden=true>&lt;fox&gt;</span> AAAA
</div></div><p>i have a footnote<sup><a id="fnr.1.0" href="#fn.1" role=doc-noteref>[1]</a></sup><sup><a id="fnr.2.0" href="#fn.2" role=doc-noteref>[2]</a></sup>
</p><p><a href="#finish-writing-this-test">i am a heading link</a>
<a href="hmm/example.org/test.html">should link to .html</a>
<a href="hmm/example.org/test.html#something">should also link to .html</a>
<a href="hmm/example.org/">im a directory!</a>
<abbr title="Yelling In Furry Form">YIFF</abbr> is an acronym
<a href="https://example.org">webbed sight</a>
</p><p><img src="https://cheapiesystems.com/media/images/libera-cat.png" alt="the libera.chat logo, but with the mountain replaced with a cat">
</p></section><h3 tabindex=-1 id="foxwash-time"><span class=todo>TODO</span> wash the fox <a class=see-focus href="#foxwash-time" aria-label="permalink to section">¶</a></h3><section><div class="chat"><img class=chat-head width=64 src="faces/fox-stimky.png" alt="fox is stimky says"><div class=chat-text><span class=chat-nick aria-hidden=true>&lt;fox&gt;</span> AAAA even more
</div></div></section><h3 tabindex=-1 id="finish-writing-this-test"><span class=done>DONE</span> finish writing this test <a class=see-focus href="#finish-writing-this-test" aria-label="permalink to section">¶</a></h3><section role=doc-endnotes aria-labelledby=clam.footnotes><h2 id=clam.footnotes>footnotes</h2><ol><li id="fn.1">beep <i>boop</i>
<a href="#fnr.1.0" role=doc-backlink>↩</a>
</li><li id="fn.2"> and *another* footnote
<a href="#fnr.2.0" role=doc-backlink>↩</a>
</li></ol></section></main>"##
        );
    }
}
