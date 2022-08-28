// TODO: Support:
// * unordered lists
// * links
// * inline code
// * code blocks
// * mentions of other pages

#[derive(Debug, PartialEq, Eq)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RichText {
    pub text: String,
    // TODO: support **strong**, _emphasis_, ~strikethrough``
}

#[derive(Debug, PartialEq, Eq)]
pub enum Tag {
    Paragraph {
        text: Vec<RichText>,
    },
    Heading {
        level: HeadingLevel,
        text: Vec<RichText>,
    },
    OrderedList {
        items: Vec<OrderedListItem>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct OrderedListItem {
    pub text: Vec<RichText>,
    pub children: Vec<Tag>,
}
