use std::collections::HashMap;
use std::io::Read;
use std::str::FromStr;
use std::{env, iter};
use std::{fs::File, io::Write};

use ::notion::ids::AsIdentifier;
use ::notion::models::ListResponse;
use ::notion::{
    ids::{BlockId, PageId},
    NotionApi,
};
use async_recursion::async_recursion;
use dotenv::dotenv;
use markdown::from_cmark::PulldownCMarkEventParser;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};

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
// 4. ✅ Read the Markdown file from disk and parse it into that structure
// 5. ✅ Recreate that parsed page in Notion (replace the entire document)
// 6. Smarter diffing: compare which parts changed and only update these documents (React-like)

const MD_FILE_NAME: &str = "test.md";

enum Action {
    Fetch,
    Sync,
}

#[tokio::main]
async fn main() {
    // NOTE: a missing `.env` file is not a problem
    dotenv().ok();

    let notion_api_key =
        env::var("NOTION_API_KEY").expect("NOTION_API_KEY environment variable to be defined");
    let mut headers = HeaderMap::new();
    headers.append(
        "Notion-Version",
        HeaderValue::from_static(
            // NOTE: same value as in the `notion` module
            "2022-02-22",
        ),
    );
    let mut auth_value =
        HeaderValue::from_str(&format!("Bearer {}", notion_api_key)).expect("valid header");
    auth_value.set_sensitive(true);
    headers.append(AUTHORIZATION, auth_value);

    let client = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .build()
        .expect("valid reqwest client");
    let notion_api = NotionApi::new(notion_api_key).expect("could not create NotionApi");
    let page_id = PageId::from_str("0b89a6e8f0064acc8ec6e6902b039e3a").expect("invalid page ID");

    let action = {
        if env::args()
            .into_iter()
            .skip(1)
            .find(|arg| arg == "sync")
            .is_some()
        {
            Action::Sync
        } else {
            Action::Fetch
        }
    };

    match action {
        Action::Fetch => {
            let mut file = File::create(MD_FILE_NAME).expect("MD file to create successfully");
            let markdown_content = convert_page_to_markdown(&notion_api, page_id).await;
            file.write_all(markdown_content.as_bytes())
                .expect("markdown content to be written to a file");
        }
        Action::Sync => {
            let mut file = File::open(MD_FILE_NAME).expect("MD file to exist");
            let mut buf = String::new();
            file.read_to_string(&mut buf)
                .expect("successfully read file");
            convert_markdown_to_page(&notion_api, &client, page_id, &buf)
                .await
                .expect("page to be synced");
        }
    }
}

async fn convert_page_to_markdown(notion_api: &NotionApi, page_id: PageId) -> String {
    let block_id: BlockId = page_id.into();
    let page_blocks = get_all_block_children(&notion_api, &block_id)
        .await
        .expect("valid blocks");

    let parsed_tags: Vec<_> = NotionToMarkdownParser::default()
        .feed(page_blocks.iter())
        .collect();

    let events = parsed_tags.iter().flat_map(get_pulldown_cmark_events);
    let mut buf = String::new();
    pulldown_cmark_to_cmark::cmark(events, &mut buf).expect("serialization failed");
    buf.push('\n');

    buf
}

async fn convert_markdown_to_page(
    notion_api: &NotionApi,
    client: &reqwest::Client,
    page_id: PageId,
    input: &str,
) -> std::io::Result<()> {
    let markdown_tags = PulldownCMarkEventParser::new(pulldown_cmark::Parser::new(input)).parse();

    erase_page(notion_api, client, page_id.clone()).await;
    create_blocks(client, &page_id.into(), &markdown_tags).await;

    Ok(())
}

// TODO: remove debug information

async fn erase_page(notion_api: &NotionApi, client: &reqwest::Client, page_id: PageId) {
    let block_id: BlockId = page_id.into();
    let child_blocks = notion_api
        .get_block_children(&block_id)
        .await
        .expect("page blocks to fetch");

    let futures = child_blocks.results().into_iter().map(|child_block| async {
        client
            .delete(format!(
                "https://api.notion.com/v1/blocks/{}",
                dbg!(child_block.as_id())
            ))
            .send()
            .await
            // TODO: return errors instead of panicing
            .expect("block to be correctly deleted")
    });
    // .collect::<FuturesUnordered<_>>()

    // NOTE: deleting all blocks in parallel skipped some blocks.
    // Possible race condition. Remove blocks sequentially as a temporary workaround.

    // TODO: delete blocks in parallel
    for future in futures {
        future.await;
    }
}

// TODO: extract this code into a separate module and improve the quality

#[async_recursion]
async fn create_blocks(
    client: &reqwest::Client,
    parent_block_id: &BlockId,
    tags: &Vec<markdown::tag::Tag>,
) {
    let append_block_url = format!(
        "https://api.notion.com/v1/blocks/{}/children",
        parent_block_id
    );

    let mock_tags: Vec<_> = tags
        .iter()
        .flat_map(|tag| -> Vec<_> {
            match tag {
                markdown::tag::Tag::Heading {
                    level: markdown::tag::HeadingLevel::H1,
                    text,
                } => iter::once(MockTag::Heading1 {
                    heading_1: rich_text_to_text(text),
                })
                .collect(),
                markdown::tag::Tag::Heading {
                    level: markdown::tag::HeadingLevel::H2,
                    text,
                } => iter::once(MockTag::Heading2 {
                    heading_2: rich_text_to_text(text),
                })
                .collect(),
                markdown::tag::Tag::Heading {
                    level: markdown::tag::HeadingLevel::H3,
                    text,
                } => iter::once(MockTag::Heading3 {
                    heading_3: rich_text_to_text(text),
                })
                .collect(),
                markdown::tag::Tag::Paragraph { text } => iter::once(MockTag::Paragraph {
                    paragraph: rich_text_to_text(text),
                })
                .collect(),

                markdown::tag::Tag::OrderedList { items } => items
                    .into_iter()
                    .map(|item| MockTag::NumberedListItem {
                        numbered_list_item: ::notion::models::TextAndChildren {
                            rich_text: item.text.iter().map(|rich_text| rich_text.into()).collect(),
                            children: Some(Vec::new()),
                            color: ::notion::models::text::TextColor::Default,
                        },
                    })
                    .collect(),
            }
        })
        .collect();

    let mut body = HashMap::new();
    body.insert("children", mock_tags);

    // TODO: return errors instead of panicing

    println!(
        "{}",
        serde_json::to_string_pretty(&body).expect("valid serialization")
    );

    let response = client
        .patch(&append_block_url)
        .json(dbg!(&body))
        .send()
        .await
        .expect("block to be created");
    dbg!(&response);
    let text = response.text().await.expect("body");
    println!("{text}");
    let deserialized_blocks: ListResponse<::notion::models::Block> =
        serde_json::from_str(&text).expect("valid block");
    dbg!(&deserialized_blocks);

    // Create nested blocks if necessary
    let mut created_blocks_iter = deserialized_blocks.results().into_iter();
    for tag in tags {
        match tag {
            markdown::tag::Tag::OrderedList { items } => {
                for item in items {
                    let created_list_item_block = created_blocks_iter
                        .next()
                        .expect("created block matching the first list item");

                    if !item.children.is_empty() {
                        create_blocks(client, created_list_item_block.as_id(), &item.children)
                            .await;
                    }
                }
            }
            _ => {
                created_blocks_iter
                    .next()
                    .expect("created block matching the parsed tag");
            }
        }
    }
}

impl From<&markdown::tag::RichText> for ::notion::models::text::RichText {
    fn from(rich_text: &markdown::tag::RichText) -> Self {
        Self::Text {
            text: ::notion::models::text::Text {
                link: None,
                content: rich_text.text.to_string(),
            },
            rich_text: ::notion::models::text::RichTextCommon {
                annotations: Some(::notion::models::text::Annotations {
                    bold: Some(false),
                    code: Some(false),
                    color: Some(::notion::models::text::TextColor::Default),
                    italic: Some(false),
                    underline: Some(false),
                    strikethrough: Some(false),
                }),
                href: None,
                plain_text: rich_text.text.to_string(),
            },
        }
    }
}

fn rich_text_to_text(text: &Vec<markdown::tag::RichText>) -> ::notion::models::Text {
    ::notion::models::Text {
        rich_text: text.iter().map(|rich_text| rich_text.into()).collect(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum MockTag {
    Heading1 {
        heading_1: ::notion::models::Text,
    },
    Heading2 {
        heading_2: ::notion::models::Text,
    },
    Heading3 {
        heading_3: ::notion::models::Text,
    },
    Paragraph {
        paragraph: ::notion::models::Text,
    },
    NumberedListItem {
        numbered_list_item: ::notion::models::TextAndChildren,
    },
}
