use std::env;
use std::io::Read;
use std::{fs::File, io::Write};

use ::notion::{
    ids::{BlockId, PageId},
    NotionApi,
};
use clap::Parser;
use cli::{Cli, Command};
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

mod cli;
mod markdown;
mod notion_api;

#[tokio::main]
async fn main() {
    // NOTE: a missing `.env` file is not a problem
    dotenv().ok();
    let cli = Cli::parse();

    let notion_api_key =
        env::var("NOTION_API_KEY").expect("NOTION_API_KEY environment variable to be defined");
    let client = NotionClient::new(get_notion_reqwest_client(&notion_api_key));
    let notion_api = NotionApi::new(notion_api_key).expect("could not create NotionApi");

    match cli.command {
        Command::Fetch { page_id, file } => {
            let mut file = File::create(file).expect("MD file to create successfully");
            let markdown_content = convert_page_to_markdown(&notion_api, page_id)
                .await
                .expect("Could not fetch the page");

            file.write_all(markdown_content.as_bytes())
                .expect("Could not write the page markdown to a file");
        }
        Command::Push { page_id, file } => {
            let mut file = File::open(file).expect("File does not exist");
            let mut buf = String::new();
            file.read_to_string(&mut buf)
                .expect("Could not read the file");
            convert_markdown_to_page(&notion_api, &client, page_id, &buf)
                .await
                .expect("Error when pushing the document to Notion");
        }
    }
}

async fn convert_page_to_markdown(
    notion_api: &NotionApi,
    page_id: PageId,
) -> Result<String, notion::Error> {
    let block_id: BlockId = page_id.into();
    let page_blocks = get_all_block_children(notion_api, &block_id).await?;

    let parsed_tags: Vec<_> = NotionToMarkdownParser::default()
        .feed(page_blocks.iter())
        .collect();

    let events = parsed_tags.iter().flat_map(get_pulldown_cmark_events);
    let mut buf = String::new();
    pulldown_cmark_to_cmark::cmark(events, &mut buf).expect("serialization failed");
    buf.push('\n');

    Ok(buf)
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
