use crate::notion::BlockWithChildren;

#[derive(Debug, Default)]
pub struct NotionToMarkdownParser {
    current_list_items: Option<Vec<super::tag::OrderedListItem>>,
}

impl NotionToMarkdownParser {
    fn parse_block(&mut self, value: &BlockWithChildren) -> Option<super::tag::Tag> {
        use super::tag::{HeadingLevel, Tag};
        use notion::models::Block;

        // TODO: ensure that children are empty in most cases
        // TODO: close lists before processing other items
        match &value.block {
            Block::Heading1 { heading_1, .. } => Some(Tag::Heading {
                level: HeadingLevel::H1,
                text: Self::parse_rich_text(&heading_1.rich_text),
            }),
            Block::Heading2 { heading_2, .. } => Some(Tag::Heading {
                level: HeadingLevel::H2,
                text: Self::parse_rich_text(&heading_2.rich_text),
            }),
            Block::Heading3 { heading_3, .. } => Some(Tag::Heading {
                level: HeadingLevel::H3,
                text: Self::parse_rich_text(&heading_3.rich_text),
            }),
            Block::Paragraph { paragraph, .. } => Some(Tag::Paragraph {
                text: Self::parse_rich_text(&paragraph.rich_text),
            }),
            Block::NumberedListItem {
                numbered_list_item, ..
            } => {
                let current_list_items = self.current_list_items.get_or_insert(Vec::new());

                let children_parser = Self::default();
                current_list_items.push(super::tag::OrderedListItem {
                    text: Self::parse_rich_text(&numbered_list_item.rich_text),
                    children: children_parser.feed(value.children.iter()).collect(),
                });

                None
            }
            _ => todo!(),
        }
    }

    fn flush(self) -> Option<super::tag::Tag> {
        if let Some(current_list_items) = self.current_list_items {
            return Some(super::tag::Tag::OrderedList {
                items: current_list_items,
            });
        }

        None
    }

    pub fn feed<'a, I>(self, blocks: I) -> MarkdownTagIterator<'a, I>
    where
        I: Iterator<Item = &'a BlockWithChildren>,
    {
        MarkdownTagIterator {
            blocks,
            parser: Some(self),
        }
    }

    fn parse_rich_text(
        rich_text: &Vec<notion::models::text::RichText>,
    ) -> Vec<super::tag::RichText> {
        rich_text.into_iter().map(Into::into).collect()
    }
}

pub struct MarkdownTagIterator<'a, I>
where
    I: Iterator<Item = &'a BlockWithChildren>,
{
    blocks: I,
    parser: Option<NotionToMarkdownParser>,
}

impl<'a, I> Iterator for MarkdownTagIterator<'a, I>
where
    I: Iterator<Item = &'a BlockWithChildren>,
{
    type Item = super::tag::Tag;

    fn next(&mut self) -> Option<Self::Item> {
        let parser = self.parser.as_mut()?;

        while let Some(block) = self.blocks.next() {
            if let Some(parsed_tag) = parser.parse_block(block) {
                return Some(parsed_tag);
            }
        }

        let parser = self.parser.take().expect("was Some before");
        return parser.flush();
    }
}

impl From<&notion::models::text::RichText> for super::tag::RichText {
    fn from(value: &notion::models::text::RichText) -> Self {
        use notion::models::text::RichText;

        Self {
            text: match value {
                RichText::Text { text, .. } => text.content.clone(),
                RichText::Equation { .. } => unimplemented!("equation"),
                RichText::Mention { .. } => todo!("mentions"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use notion::{
        ids::{BlockId, UserId},
        models::{
            text::{Annotations, Link, RichTextCommon, TextColor},
            users::UserCommon,
            Block, BlockCommon, TextAndChildren,
        },
    };

    use crate::markdown::tag::{OrderedListItem, Tag};

    use super::*;

    fn get_block_common_stub() -> BlockCommon {
        let user_common = UserCommon {
            id: UserId::from_str("ac32e0256f9c4fab8b9ddbb3c593ac46").expect("valid user ID"),
            name: Some("Username".to_string()),
            avatar_url: None,
        };

        BlockCommon {
            id: BlockId::from_str("ac32e0256f9c4fab8b9ddbb3c593ac46").expect("valid block ID"),
            created_time: Default::default(),
            last_edited_time: Default::default(),
            has_children: true,
            created_by: user_common.clone(),
            last_edited_by: user_common,
        }
    }

    fn get_rich_text(
        plain_text: &str,
        link: Option<Link>,
        annotations: Option<Annotations>,
    ) -> notion::models::text::RichText {
        notion::models::text::RichText::Text {
            rich_text: RichTextCommon {
                plain_text: plain_text.to_string(),
                href: link.as_ref().map(|link| link.url.clone()),
                annotations,
            },
            text: notion::models::text::Text {
                content: plain_text.to_string(),
                link,
            },
        }
    }

    fn get_default_annotations() -> Annotations {
        Annotations {
            strikethrough: None,
            bold: None,
            code: None,
            color: Some(TextColor::Default),
            italic: None,
            underline: None,
        }
    }

    fn get_numbered_list_item(rich_text: Vec<notion::models::text::RichText>) -> Block {
        Block::NumberedListItem {
            common: get_block_common_stub(),
            numbered_list_item: TextAndChildren {
                color: TextColor::Default,
                rich_text,
                children: None,
            },
        }
    }

    #[test]
    fn parses_simple_doc() {
        let blocks = vec![
            BlockWithChildren {
                block: Block::Heading1 {
                    common: get_block_common_stub(),
                    heading_1: notion::models::Text {
                        rich_text: vec![get_rich_text(
                            "Summary",
                            None,
                            Some(get_default_annotations()),
                        )],
                    },
                },
                children: Vec::new(),
            },
            BlockWithChildren {
                block: get_numbered_list_item(vec![get_rich_text(
                    "Watch some videos",
                    None,
                    Some(get_default_annotations()),
                )]),
                children: Vec::new(),
            },
            BlockWithChildren {
                block: get_numbered_list_item(vec![get_rich_text(
                    "Another list item",
                    None,
                    Some(get_default_annotations()),
                )]),
                children: vec![BlockWithChildren {
                    block: get_numbered_list_item(vec![get_rich_text(
                        "Second level list item",
                        None,
                        Some(get_default_annotations()),
                    )]),
                    children: vec![BlockWithChildren {
                        block: Block::Paragraph {
                            common: get_block_common_stub(),
                            paragraph: TextAndChildren {
                                rich_text: vec![get_rich_text(
                                    "Second level item's extra description",
                                    None,
                                    Some(get_default_annotations()),
                                )],
                                children: None,
                                color: TextColor::Default,
                            },
                        },
                        children: Vec::new(),
                    }],
                }],
            },
            BlockWithChildren {
                block: Block::Heading1 {
                    common: get_block_common_stub(),
                    heading_1: notion::models::Text {
                        rich_text: vec![get_rich_text(
                            "Details",
                            None,
                            Some(get_default_annotations()),
                        )],
                    },
                },
                children: Vec::new(),
            },
            BlockWithChildren {
                block: Block::Paragraph {
                    common: get_block_common_stub(),
                    paragraph: TextAndChildren {
                        rich_text: vec![get_rich_text(
                            "More description",
                            None,
                            Some(get_default_annotations()),
                        )],
                        children: None,
                        color: TextColor::Default,
                    },
                },
                children: Vec::new(),
            },
        ];
        let parser = NotionToMarkdownParser::default();

        let result: Vec<_> = parser.feed(blocks.iter()).collect();

        assert_eq!(
            result,
            vec![
                Tag::Heading {
                    level: crate::markdown::tag::HeadingLevel::H1,
                    text: vec![crate::markdown::tag::RichText {
                        text: "Summary".to_string()
                    }]
                },
                Tag::OrderedList {
                    items: vec![
                        OrderedListItem {
                            text: vec![crate::markdown::tag::RichText {
                                text: "Watch some videos".to_string()
                            }],
                            children: Vec::new(),
                        },
                        OrderedListItem {
                            text: vec![crate::markdown::tag::RichText {
                                text: "Another list item".to_string()
                            }],
                            children: vec![Tag::OrderedList {
                                items: vec![OrderedListItem {
                                    text: vec![crate::markdown::tag::RichText {
                                        text: "Second level list item".to_string()
                                    }],
                                    children: vec![Tag::Paragraph {
                                        text: vec![crate::markdown::tag::RichText {
                                            text: "Second level item's extra description"
                                                .to_string()
                                        }]
                                    }]
                                }]
                            }],
                        }
                    ]
                },
                Tag::Heading {
                    level: crate::markdown::tag::HeadingLevel::H1,
                    text: vec![crate::markdown::tag::RichText {
                        text: "Details".to_string()
                    }]
                },
                Tag::Paragraph {
                    text: vec![crate::markdown::tag::RichText {
                        text: "More description".to_string()
                    }]
                }
            ]
        )
    }
}
