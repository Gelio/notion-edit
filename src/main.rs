use std::env;
use std::io::Read;
use std::str::FromStr;
use std::{fs::File, io::Write};

use ::notion::{
    ids::{BlockId, PageId},
    NotionApi,
};
use dotenv::dotenv;
use markdown::from_cmark::{ParseError, PulldownCMarkEventParser};
use markdown::notion_interop::NotionToMarkdownParser;
use markdown::to_cmark::get_pulldown_cmark_events;
use notion::ids::AsIdentifier;
use notion_api::client::{
    erase_page, get_all_block_children, get_notion_reqwest_client, AppendBlockChildrenError,
    BlockWithChildrenToCreate, ErasePageError, NotionClient,
};
use thiserror::Error;

mod markdown;
mod notion_api;

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
    let client = NotionClient::new(get_notion_reqwest_client(&notion_api_key));
    let notion_api = NotionApi::new(notion_api_key).expect("could not create NotionApi");
    // TODO: get the page ID from CLI arguments
    let page_id = PageId::from_str("0b89a6e8f0064acc8ec6e6902b039e3a").expect("invalid page ID");

    let action = {
        // TODO: get the action from CLI arguments
        if env::args().into_iter().skip(1).any(|arg| arg == "sync") {
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
    let page_blocks = get_all_block_children(notion_api, &block_id)
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

#[derive(Error, Debug)]
enum MarkdownToPageError<'a> {
    #[error("cannot erase page {0}")]
    ErasePage(#[from] ErasePageError),

    #[error("cannot create blocks")]
    CreateBlocks(Vec<AppendBlockChildrenError>),

    #[error("cannot parse document {0}")]
    Parse(ParseError<'a>),
}

async fn convert_markdown_to_page<'a>(
    notion_api: &NotionApi,
    client: &NotionClient,
    page_id: PageId,
    input: &'a str,
) -> Result<(), MarkdownToPageError<'a>> {
    erase_page(notion_api, client, page_id.clone()).await?;

    let markdown_tags = PulldownCMarkEventParser::new(pulldown_cmark::Parser::new(input))
        .parse()
        .map_err(MarkdownToPageError::Parse)?;
    let blocks_to_create: Vec<_> = markdown_tags
        .into_iter()
        .flat_map(BlockWithChildrenToCreate::from_markdown_tag)
        .collect();
    client
        .create_blocks(page_id.as_id().clone().into(), blocks_to_create)
        .await
        .map_err(MarkdownToPageError::CreateBlocks)?;

    Ok(())
}
