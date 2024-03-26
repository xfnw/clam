use orgize::export::{Container, Event, HtmlEscape, HtmlExport, TraversalContext, Traverser};
use slugify::slugify;
use std::cmp::min;

#[derive(Default)]
pub struct Handler(pub HtmlExport);

impl Traverser for Handler {
    fn event(&mut self, event: Event, ctx: &mut TraversalContext) {
        match event {
            Event::Enter(Container::Headline(headline)) => {
                let lvl = headline.level();
                let lead = "#".repeat(lvl);
                let lvl = 1 + min(lvl, 5);
                let txt = headline.title().map(|t| t.to_string()).collect::<String>();

                self.0.push_str(format!(
                    r##"<h{} id="{1}"><a role=none href="#{1}">{2}</a> "##,
                    lvl,
                    slugify!(&txt),
                    lead
                ));

                for e in headline.title() {
                    self.element(e, ctx);
                }

                self.0.push_str(format!("</h{}>", lvl));
            }
            // why does the default HtmlExport output keywords with literally
            // zero formatting? no idea! let's instead not output them at all.
            Event::Enter(Container::Keyword(_)) => ctx.skip(),
            Event::Enter(Container::Link(link)) => {
                let path = link.path();
                let path = path.trim_start_matches("file:");
                // FIXME: breaks if linking to bare .org domain.
                // hopefully most have a trailing slash?
                let path = if let Some(p) = path.strip_suffix(".org") {
                    let mut p = p.to_string();
                    p.push_str(".html");
                    p
                } else if path.contains(".org#") {
                    path.replace(".org#", ".html#")
                } else {
                    path.to_string()
                };

                if link.is_image() {
                    // FIXME: needs alt text support
                    self.0
                        .push_str(format!("<img src=\"{}\">", HtmlEscape(&path)));
                    return ctx.skip();
                }

                self.0
                    .push_str(format!("<a href=\"{}\">", HtmlEscape(&path)));

                if !link.has_description() {
                    self.0.push_str(format!("{}</a>", HtmlEscape(&path)));
                    ctx.skip();
                }
            }
            Event::Enter(Container::Subscript(_)) => self.0.push_str("_"),
            Event::Leave(Container::Subscript(_)) => (),
            _ => self.0.event(event, ctx),
        };
    }
}
