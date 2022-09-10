use std::str::FromStr;

use ::notion::{
    ids::{BlockId, PageId},
    NotionApi,
};
use dotenv::dotenv;

use crate::{
    markdown::{notion_interop::NotionToMarkdownParser, to_cmark::get_pulldown_cmark_events},
    notion::get_all_block_children,
};

mod markdown;
mod notion;

// TODO:
// 1. ✅ Fetch the page and all its blocks (probably use the API directly, avoid the notion_rs crate)
//
// 2. ✅ Parse the page into a markdown document.
//      Supported nodes:
//      1. Numbered lists
//      2. Paragraphs
//      3. Headings
//
//      Later, add support for:
//      1. Bold text, italics
//      2. Inline code
//      3. References to other Notion pages
//      4. External links
//      5. Code blocks
//
// 3. ✅ Write that parsed page into a Markdown file
// 4. Read the Markdown file from disk and parse it into that structure
// 5. Recreate that parsed page in Notion (replace the entire document)
// 6. Smarter diffing: compare which parts changed and only update these documents (React-like)

#[tokio::main]
async fn main() {
    // NOTE: a missing `.env` file is not a problem
    dotenv().ok();

    let notion_api_key =
        std::env::var("NOTION_API_KEY").expect("NOTION_API_KEY environment variable to be defined");
    let notion_api = NotionApi::new(notion_api_key).expect("could not create NotionApi");

    let page_id = PageId::from_str("0b89a6e8f0064acc8ec6e6902b039e3a").expect("invalid page ID");

    let block_id: BlockId = page_id.into();
    let page_blocks = get_all_block_children(&notion_api, &block_id)
        .await
        .expect("valid blocks");
    println!("{:#?}", page_blocks);

    let parsed_tags: Vec<_> = NotionToMarkdownParser::default()
        .feed(page_blocks.iter())
        .collect();
    println!("Tags: {parsed_tags:#?}");

    let events = parsed_tags.iter().flat_map(get_pulldown_cmark_events);
    let mut buf = String::new();
    pulldown_cmark_to_cmark::cmark(events, &mut buf).expect("serialization failed");

    println!("{buf}");
}
