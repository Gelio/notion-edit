use std::iter::Peekable;

use pulldown_cmark::Event;

pub struct PulldownCMarkEventParser<I> {
    event_iterator: I,
}

impl<'a, I> PulldownCMarkEventParser<Peekable<I>>
where
    I: Iterator<Item = Event<'a>>,
{
    pub fn new(event_iterator: I) -> Self {
        Self {
            event_iterator: event_iterator.peekable(),
        }
    }

    pub fn parse(mut self) -> Vec<super::tag::Tag> {
        let mut tags: Vec<super::tag::Tag> = Vec::new();

        while let Some(event) = self.event_iterator.next() {
            tags.push(self.parse_single_event(event));
        }

        tags
    }

    fn parse_single_event(&mut self, event: Event) -> super::tag::Tag {
        match event {
            Event::Start(tag) => match tag {
                pulldown_cmark::Tag::Heading(original_heading_level, None, _) => {
                    self.parse_heading(original_heading_level)
                }
                pulldown_cmark::Tag::List(Some(_start_number)) => {
                    let mut items: Vec<super::tag::OrderedListItem> = Vec::new();

                    while self
                        .event_iterator
                        .next_if_eq(&pulldown_cmark::Event::Start(pulldown_cmark::Tag::Item))
                        .is_some()
                    {
                        items.push(self.parse_ordered_list_item());
                    }

                    assert_eq!(
                        self.event_iterator.next().expect("end of list"),
                        Event::End(tag),
                        "end of list tag"
                    );

                    super::tag::Tag::OrderedList { items }
                }
                pulldown_cmark::Tag::Paragraph => self.parse_paragraph(),
                tag => unimplemented!("tag type {tag:?} not implemented"),
            },
            Event::End(_) => {
                unreachable!(
                    "end events should be handled in start event handlers, found {event:#?}"
                )
            }
            Event::Text(_) => {
                unreachable!("text should be handled in start event handlers, found {event:#?}")
            }
            event => unimplemented!("unhandled event: {event:?}"),
        }
    }

    fn parse_heading(
        &mut self,
        original_heading_level: pulldown_cmark::HeadingLevel,
    ) -> super::tag::Tag {
        let heading_level = match original_heading_level {
            pulldown_cmark::HeadingLevel::H1 => super::tag::HeadingLevel::H1,
            pulldown_cmark::HeadingLevel::H2 => super::tag::HeadingLevel::H2,
            pulldown_cmark::HeadingLevel::H3 => super::tag::HeadingLevel::H3,
            _ => unimplemented!("heading level not supported by Notion"),
        };

        // TODO: handle text modifiers
        // TODO: handle multiple text events
        let text = match self.event_iterator.next().expect("non empty text") {
            Event::Text(text) => text,
            event => unreachable!("heading is immediately followed by text, found {event:#?}"),
        };
        match self.event_iterator.next().expect("non empty end tag") {
            Event::End(pulldown_cmark::Tag::Heading(..)) => {}
            event => unreachable!(
                "start heading should have a matching end heading event, found {event:#?}"
            ),
        }

        super::tag::Tag::Heading {
            level: heading_level,
            text: vec![super::tag::RichText {
                text: text.to_string(),
            }],
        }
    }

    fn parse_ordered_list_item(&mut self) -> super::tag::OrderedListItem {
        match self.event_iterator.next().expect("paragraph for list item") {
            Event::Start(pulldown_cmark::Tag::Paragraph) => {}
            _ => unreachable!("list items must have paragraph immediately inside"),
        }

        // TODO: handle text modifiers
        // TODO: handle multiple text events
        let text = match self.event_iterator.next().expect("text inside paragraph") {
            Event::Text(text) => text,
            event => unreachable!("paragraph is immediately followed by text, not by {event:?}"),
        };

        assert_eq!(
            self.event_iterator.next(),
            Some(Event::End(pulldown_cmark::Tag::Paragraph)),
            "end of paragraph"
        );

        let end_item_event = Event::End(pulldown_cmark::Tag::Item);
        let mut children: Vec<super::tag::Tag> = Vec::new();

        loop {
            let event = self
                .event_iterator
                .next()
                .expect("no abrupt end of events - the end item event should still appear");

            if event == end_item_event {
                break;
            }
            children.push(self.parse_single_event(event));
        }

        let list_item = super::tag::OrderedListItem {
            text: vec![super::tag::RichText {
                text: text.to_string(),
            }],
            children,
        };

        list_item
    }

    fn parse_paragraph(&mut self) -> super::tag::Tag {
        // TODO: handle text modifiers
        // TODO: handle multiple text events
        let text = match self.event_iterator.next().expect("text inside paragraph") {
            Event::Text(text) => text,
            event => unreachable!("paragraph is immediately followed by text, not by {event:?}"),
        };

        assert_eq!(
            self.event_iterator.next(),
            Some(Event::End(pulldown_cmark::Tag::Paragraph)),
            "end of paragraph"
        );

        super::tag::Tag::Paragraph {
            text: vec![super::tag::RichText {
                text: text.to_string(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::markdown::tag::{OrderedListItem, Tag};

    use super::*;

    fn get_document_tags() -> Vec<Tag> {
        vec![
            Tag::Heading {
                level: crate::markdown::tag::HeadingLevel::H1,
                text: vec![crate::markdown::tag::RichText {
                    text: "Summary".to_string(),
                }],
            },
            Tag::OrderedList {
                items: vec![
                    OrderedListItem {
                        text: vec![crate::markdown::tag::RichText {
                            text: "Watch some videos".to_string(),
                        }],
                        children: Vec::new(),
                    },
                    OrderedListItem {
                        text: vec![crate::markdown::tag::RichText {
                            text: "Another list item".to_string(),
                        }],
                        children: vec![Tag::OrderedList {
                            items: vec![OrderedListItem {
                                text: vec![crate::markdown::tag::RichText {
                                    text: "Second level list item".to_string(),
                                }],
                                children: vec![Tag::Paragraph {
                                    text: vec![crate::markdown::tag::RichText {
                                        text: "Second level item's extra description".to_string(),
                                    }],
                                }],
                            }],
                        }],
                    },
                ],
            },
            Tag::Heading {
                level: crate::markdown::tag::HeadingLevel::H1,
                text: vec![crate::markdown::tag::RichText {
                    text: "Details".to_string(),
                }],
            },
            Tag::Paragraph {
                text: vec![crate::markdown::tag::RichText {
                    text: "More description".to_string(),
                }],
            },
        ]
    }

    const SERIALIZED_DOCUMENT: &str = r"# Summary

1. Watch some videos

1. Another list item
   
   1. Second level list item
      
      Second level item's extra description

# Details

More description";

    #[test]
    fn parses_as_expected() {
        let mut event_parser = pulldown_cmark::Parser::new(SERIALIZED_DOCUMENT);
        let parsed_document = PulldownCMarkEventParser::new(&mut event_parser).parse();

        assert_eq!(
            parsed_document,
            get_document_tags(),
            "different parse result"
        );
    }
}
