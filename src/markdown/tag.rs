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
    // https://github.com/Gelio/notion-edit/issues/1
}

#[derive(Debug, PartialEq, Eq)]
pub struct Paragraph {
    pub text: Vec<RichText>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Tag {
    Paragraph(Paragraph),
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
