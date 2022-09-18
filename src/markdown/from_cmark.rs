use std::iter::Peekable;

use pulldown_cmark::Event;
use thiserror::Error;

pub struct PulldownCMarkEventParser<I> {
    event_iterator: I,
}

#[derive(Debug, Error)]
pub enum ParseError<'a> {
    #[error("unexpected heading level {0}, Notion only supports heading levels up to 3")]
    UnexpectedHeadingLevel(pulldown_cmark::HeadingLevel),

    #[error("unimplemented tag")]
    UnimplementedTag(pulldown_cmark::Tag<'a>),
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

    pub fn parse(mut self) -> Result<Vec<super::tag::Tag>, ParseError<'a>> {
        let mut tags: Vec<super::tag::Tag> = Vec::new();

        while let Some(event) = self.event_iterator.next() {
            tags.push(self.parse_single_event(event)?);
        }

        Ok(tags)
    }

    fn parse_single_event(&mut self, event: Event<'a>) -> Result<super::tag::Tag, ParseError<'a>> {
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
                        items.push(self.parse_ordered_list_item()?);
                    }

                    assert_eq!(
                        self.event_iterator.next().expect("end of list"),
                        Event::End(tag),
                        "end of list tag"
                    );

                    Ok(super::tag::Tag::OrderedList { items })
                }
                pulldown_cmark::Tag::Paragraph => {
                    Ok(super::tag::Tag::Paragraph(self.parse_paragraph()))
                }
                tag => Err(ParseError::UnimplementedTag(tag)),
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

    /// Parses a markdown heading.
    /// Assumes the Event::Start(Heading) event was already consumed.
    fn parse_heading(
        &mut self,
        original_heading_level: pulldown_cmark::HeadingLevel,
    ) -> Result<super::tag::Tag, ParseError<'a>> {
        let heading_level = match original_heading_level {
            pulldown_cmark::HeadingLevel::H1 => Ok(super::tag::HeadingLevel::H1),
            pulldown_cmark::HeadingLevel::H2 => Ok(super::tag::HeadingLevel::H2),
            pulldown_cmark::HeadingLevel::H3 => Ok(super::tag::HeadingLevel::H3),
            _ => Err(ParseError::UnexpectedHeadingLevel(original_heading_level)),
        }?;

        let text = self.parse_text();
        assert!(!text.is_empty(), "empty heading");

        match self
            .event_iterator
            .next()
            .expect("unexpected end of events, expected end of heading")
        {
            Event::End(pulldown_cmark::Tag::Heading(..)) => {}
            event => unreachable!(
                "start heading should have a matching end heading event, found {event:#?}"
            ),
        }

        Ok(super::tag::Tag::Heading {
            level: heading_level,
            text,
        })
    }

    /// Parses an ordered list item with its content.
    /// Assumes the start event for the list item was already consumed.
    fn parse_ordered_list_item(&mut self) -> Result<super::tag::OrderedListItem, ParseError<'a>> {
        let paragraph = match self
            .event_iterator
            .peek()
            .expect("unexpected end of events, expected list item to have some content")
        {
            Event::Start(pulldown_cmark::Tag::Paragraph) => {
                self.event_iterator
                    .next()
                    .expect("the start of the paragraph was just peeked, so it must exist");
                self.parse_paragraph()
            }
            Event::Text(_) => super::tag::Paragraph {
                text: self.parse_text(),
            },

            event => {
                unreachable!(
                    "list items must have paragraph or text immediately inside, found {event:#?}"
                )
            }
        };

        let end_item_event = Event::End(pulldown_cmark::Tag::Item);
        let mut children: Vec<super::tag::Tag> = Vec::new();

        loop {
            let event = self
                .event_iterator
                .next()
                .expect("abrupt end of events - the end item event should still appear");

            if event == end_item_event {
                break;
            }
            children.push(self.parse_single_event(event)?);
        }

        Ok(super::tag::OrderedListItem {
            text: paragraph.text,
            children,
        })
    }

    /// Parses a markdown paragraph.
    /// Assumes the Event::Start(Paragraph) was already consumed.
    fn parse_paragraph(&mut self) -> super::tag::Paragraph {
        let text = self.parse_text();
        assert!(!text.is_empty(), "empty paragraph");

        assert_eq!(
            self.event_iterator.next(),
            Some(Event::End(pulldown_cmark::Tag::Paragraph)),
            "end of paragraph"
        );

        super::tag::Paragraph { text }
    }

    /// Parses Event::Text until another type of event is encountered.{
    fn parse_text(&mut self) -> Vec<super::tag::RichText> {
        // TODO: handle text modifiers
        // https://github.com/Gelio/notion-edit/issues/1

        let mut parsed_text = Vec::new();
        while let Some(Event::Text(text)) = self.event_iterator.peek() {
            parsed_text.push(super::tag::RichText {
                text: text.to_string(),
            });

            // NOTE: consume the peeked event
            self.event_iterator.next();
        }

        parsed_text
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::markdown::tag::{OrderedListItem, Paragraph, Tag};

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
                                children: vec![Tag::Paragraph(Paragraph {
                                    text: vec![crate::markdown::tag::RichText {
                                        text: "Second level item's extra description".to_string(),
                                    }],
                                })],
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
            Tag::Paragraph(Paragraph {
                text: vec![crate::markdown::tag::RichText {
                    text: "More description".to_string(),
                }],
            }),
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
            parsed_document.unwrap(),
            get_document_tags(),
            "different parse result"
        );
    }

    #[test]
    fn parses_larger_document() {
        let mut event_parser = pulldown_cmark::Parser::new(
            "# Summary

1. Something to do

1. Here
   
   1. Second level list item
      
      Second level list item’s paragraph
      
      1. Third level list

## Subheading

1. Done something

1. Done something else

1. New point",
        );

        PulldownCMarkEventParser::new(&mut event_parser)
            .parse()
            .expect("successful parsing");
    }
}
