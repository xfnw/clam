use orgize::{
    ast::{PropertyDrawer, TodoType},
    export::{Container, Event, HtmlEscape, HtmlExport, TraversalContext, Traverser},
};
use rowan::ast::{support, AstNode};
use slugify::slugify;
use std::cmp::min;

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
                let lead = "#".repeat(lvl);
                let lvl = 1 + min(lvl, 5);
                let txt = headline.title().map(|t| t.to_string()).collect::<String>();

                let id = if let Some(Some(cid)) =
                    support::children::<PropertyDrawer>(headline.syntax())
                        .next()
                        .map(|p| p.get("CUSTOM_ID"))
                {
                    cid.to_string()
                } else {
                    slugify!(&txt)
                };

                self.exp.push_str(format!(
                    r##"<h{} id="{1}"><a role=none href="#{1}">{2}</a> "##,
                    lvl, id, lead
                ));

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

                for e in headline.title() {
                    self.element(e, ctx);
                }

                self.exp.push_str(format!("</h{}>", lvl));
            }
            // why does the default HtmlExport output keywords with literally
            // zero formatting? no idea! let's instead not output them at all.
            Event::Enter(Container::Keyword(_)) => ctx.skip(),
            Event::Enter(Container::Link(link)) => {
                let path = link.path();
                let path = path.trim_start_matches("file:");
                let path = if let Some(p) = path.strip_prefix('*') {
                    let mut p = slugify!(p);
                    p.insert(0, '#');
                    p
                // FIXME: breaks if linking to bare .org domain.
                // hopefully most have a trailing slash?
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
                                    self.exp.push_str(
                                        "<img class=chat-head aria-hidden=true width=42 src=\"",
                                    );
                                    for _ in 1..self.numdir {
                                        self.exp.push_str("../");
                                    }
                                    self.exp.push_str(format!(
					r#"faces/{}.png"><span class=chat-nick aria-label="{1} says">&lt;{1}&gt;</span> "#,
					slugify!(usr), HtmlEscape(usr.rsplit_once('/').map_or(usr, |u| u.0))
				    ));
                                }
                            }
                        }

                        self.output_block_children(block, ctx);

                        self.exp.push_str("</div>");
                    }
                }
                ctx.skip();
            }
            Event::Enter(Container::Subscript(_)) => self.exp.push_str("_"),
            Event::Leave(Container::Subscript(_)) => (),
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
}
