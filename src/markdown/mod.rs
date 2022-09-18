pub mod from_cmark;
pub mod notion_interop;
pub mod tag;
pub mod to_cmark;

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{from_cmark::PulldownCMarkEventParser, to_cmark::get_pulldown_cmark_events};

    fn assert_parse_and_serialize(document: &str) {
        let mut event_parser = pulldown_cmark::Parser::new(document);
        let parsed_document = PulldownCMarkEventParser::new(&mut event_parser)
            .parse()
            .expect("successful parsing of the document");

        let events = parsed_document.iter().flat_map(get_pulldown_cmark_events);
        let mut buf = String::new();
        pulldown_cmark_to_cmark::cmark(events, &mut buf).expect("serialization to pass");

        assert_eq!(
            buf, document,
            "parse and serialize yields the same document"
        );
    }

    #[test]
    fn simple_document_with_list() {
        assert_parse_and_serialize(
            r"# Summary

1. Watch some videos

1. Another list item
   
   1. Second level list item
      
      Second level item's extra description

# Details

More description",
        );
    }

    #[test]
    fn text_with_decorations() {
        assert_parse_and_serialize(
            r"# _Decorated_ summary

A **decorated** _paragraph_ with ~strikethrough~ and **_mixed_** decorations.

1. *List items can have decorations **too***",
        );
    }
}
