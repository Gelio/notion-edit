use pulldown_cmark::{CowStr, Event};

pub fn get_pulldown_cmark_events(tag: &super::tag::Tag) -> Vec<Event> {
    match tag {
        super::tag::Tag::Heading { level, text } => {
            let tag = pulldown_cmark::Tag::Heading(level.into(), None, Vec::new());

            let mut events = Vec::with_capacity(text.len() + 2);
            events.push(Event::Start(tag.clone()));
            events.extend(rich_text_to_events(text));
            events.push(Event::End(tag));
            events
        }
        super::tag::Tag::Paragraph(super::tag::Paragraph { text }) => {
            let tag = pulldown_cmark::Tag::Paragraph;

            let mut events = Vec::with_capacity(text.len() + 2);
            events.push(Event::Start(tag.clone()));
            events.extend(rich_text_to_events(text));
            events.push(Event::End(tag));
            events
        }
        super::tag::Tag::OrderedList { items } => {
            let list_tag = pulldown_cmark::Tag::List(Some(1));

            let mut events: Vec<Event> = Vec::with_capacity(items.len() + 2);
            events.push(Event::Start(list_tag.clone()));

            for item in items {
                let list_item_tag = pulldown_cmark::Tag::Item;
                events.push(Event::Start(list_item_tag.clone()));

                {
                    let paragraph_tag = pulldown_cmark::Tag::Paragraph;
                    events.push(Event::Start(paragraph_tag.clone()));
                    events.extend(rich_text_to_events(&item.text));
                    events.push(Event::End(paragraph_tag));
                }

                for child in &item.children {
                    events.extend(get_pulldown_cmark_events(child));
                }
                events.push(Event::End(list_item_tag));
            }
            events.push(Event::End(list_tag));
            events
        }
    }
}

fn rich_text_to_events(text_parts: &[super::tag::RichText]) -> impl Iterator<Item = Event> {
    text_parts.iter().flat_map(Into::<Vec<_>>::into)
}

impl From<&super::tag::HeadingLevel> for pulldown_cmark::HeadingLevel {
    fn from(value: &super::tag::HeadingLevel) -> Self {
        match value {
            super::tag::HeadingLevel::H1 => Self::H1,
            super::tag::HeadingLevel::H2 => Self::H2,
            super::tag::HeadingLevel::H3 => Self::H3,
        }
    }
}

impl<'a> From<&'a super::tag::RichText> for Vec<pulldown_cmark::Event<'a>> {
    fn from(value: &'a super::tag::RichText) -> Self {
        vec![Event::Text(CowStr::Borrowed(&value.text))]
    }
}

#[cfg(test)]
mod tests {
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
    fn prints_simple_document() {
        let document = get_document_tags();

        let events = document.iter().flat_map(get_pulldown_cmark_events);

        let mut buf = String::new();
        pulldown_cmark_to_cmark::cmark(events, &mut buf).expect("serialization failed");
        assert_eq!(buf, SERIALIZED_DOCUMENT);
    }

    #[test]
    fn same_events_as_pulldown_cmark() {
        let document = get_document_tags();
        let document_events: Vec<_> = document
            .iter()
            .flat_map(get_pulldown_cmark_events)
            .collect();

        let parser = pulldown_cmark::Parser::new(SERIALIZED_DOCUMENT);
        let parsed_events: Vec<_> = parser.collect();

        assert_eq!(document_events, parsed_events, "different events");
    }
}
