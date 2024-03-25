use orgize::export::{Container, Event, HtmlExport, TraversalContext, Traverser};
use slugify::slugify;
use std::cmp::min;

#[derive(Default)]
pub struct Handler(pub HtmlExport);

impl Traverser for Handler {
    fn event(&mut self, event: Event, ctx: &mut TraversalContext) {
        match event {
            Event::Enter(Container::Headline(headline)) => {
                let lvl = 1 + min(headline.level(), 5);
                let txt = headline.title().map(|t| t.to_string()).collect::<String>();

                self.0
                    .push_str(format!("<h{} id=\"{}\">", lvl, slugify!(&txt)));

                for e in headline.title() {
                    self.element(e, ctx);
                }

                self.0.push_str(format!("</h{}>", lvl));
            }
            // why does the default HtmlExport output keywords with literally
            // zero formatting? no idea! let's instead not output them at all.
            Event::Enter(Container::Keyword(_)) => ctx.skip(),
            _ => self.0.event(event, ctx),
        };
    }
}
