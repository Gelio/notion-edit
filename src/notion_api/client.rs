use async_recursion::async_recursion;
use futures::future::join_all;
use notion::{
    ids::{AsIdentifier, BlockId, PageId},
    models::ListResponse,
};
use reqwest::header;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::markdown::tag::{HeadingLevel, Paragraph, Tag};

use super::{BlockWithChildren, GetCommon};

pub struct NotionClient {
    client: reqwest::Client,
}

#[derive(Error, Debug)]
pub enum AppendBlockChildrenError {
    #[error("cannot append block children")]
    AppendFailed {
        error: reqwest::Error,
        parent_block_id: BlockId,
        children: Vec<BlockToCreate>,
    },

    #[error("response cannot be deserialized")]
    UnexpectedResponse { error: reqwest::Error },
}

impl NotionClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn delete_block(&self, block_id: BlockId) -> Result<(), reqwest::Error> {
        self.client
            .delete(format!("https://api.notion.com/v1/blocks/{}", block_id))
            .send()
            .await
            .map(|_| ())
    }

    async fn append_block_children_shallow<Children>(
        &self,
        parent_block_id: BlockId,
        children: Children,
    ) -> Result<Vec<notion::models::Block>, AppendBlockChildrenError>
    where
        Children: IntoIterator<Item = BlockToCreate>,
    {
        let append_block_children_url = format!(
            "https://api.notion.com/v1/blocks/{}/children",
            parent_block_id
        );
        let children: Vec<_> = children.into_iter().collect();

        let created_blocks = self
            .client
            .patch(append_block_children_url)
            .json(&children)
            .send()
            .await
            .map_err(|error| AppendBlockChildrenError::AppendFailed {
                error,
                children,
                parent_block_id,
            })?
            .json::<ListResponse<notion::models::Block>>()
            .await
            .map_err(|error| AppendBlockChildrenError::UnexpectedResponse { error })?
            .results;

        Ok(created_blocks)
    }

    #[async_recursion]
    pub async fn create_blocks<'a>(
        &self,
        parent_block_id: BlockId,
        blocks_to_create: Vec<BlockWithChildrenToCreate>,
    ) -> Result<(), Vec<AppendBlockChildrenError>> {
        let mut blocks_to_create_iter = blocks_to_create.into_iter();
        let top_level_blocks_to_create: Vec<_> = blocks_to_create_iter
            .by_ref()
            .map(|block_to_create| block_to_create.block)
            .collect();

        let created_blocks = self
            .append_block_children_shallow(parent_block_id.clone(), top_level_blocks_to_create)
            .await
            .map_err(|error| vec![error])?;

        join_all(std::iter::zip(blocks_to_create_iter, created_blocks).map(
            |(block_to_create, created_block)| async move {
                if block_to_create.children.is_empty() {
                    Ok(())
                } else {
                    self.create_blocks(created_block.as_id().clone(), block_to_create.children)
                        .await
                }
            },
        ))
        .await
        .into_iter()
        .collect::<Result<_, _>>()
    }
}

pub fn get_notion_reqwest_client(notion_api_key: &str) -> reqwest::Client {
    let mut headers = header::HeaderMap::new();
    headers.append(
        "Notion-Version",
        header::HeaderValue::from_static(
            // NOTE: same value as in the `notion` module
            // https://github.com/jakeswenson/notion/blob/e75a5433a98ce51c1fe1633ee5344879c01e7fb7/src/lib.rs#L15
            "2022-02-22",
        ),
    );

    let mut auth_value =
        header::HeaderValue::from_str(&format!("Bearer {}", notion_api_key)).expect("valid header");
    auth_value.set_sensitive(true);
    headers.append(header::AUTHORIZATION, auth_value);

    reqwest::ClientBuilder::new()
        .default_headers(headers)
        .build()
        .expect("valid reqwest client")
}

// NOTE: the Block enum from the notion crate requires too much boilerplate information
// that is not required to create the block.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockToCreate {
    Heading1 {
        heading_1: notion::models::Text,
    },
    Heading2 {
        heading_2: notion::models::Text,
    },
    Heading3 {
        heading_3: notion::models::Text,
    },
    Paragraph {
        paragraph: notion::models::Text,
    },
    NumberedListItem {
        numbered_list_item: notion::models::TextAndChildren,
    },
}

pub struct BlockWithChildrenToCreate {
    block: BlockToCreate,
    children: Vec<BlockWithChildrenToCreate>,
}

impl BlockWithChildrenToCreate {
    fn childless_block(block: BlockToCreate) -> Self {
        Self {
            block,
            children: Vec::new(),
        }
    }
    /// Transforms the tag into a vector of blocks to create.
    ///
    /// It does not handle nested children (for example for lists).
    /// The consumer should go through
    pub fn from_markdown_tag(tag: Tag) -> Vec<Self> {
        match tag {
            Tag::Heading {
                level: HeadingLevel::H1,
                text,
            } => vec![Self::childless_block(BlockToCreate::Heading1 {
                heading_1: text.into_iter().collect(),
            })],
            Tag::Heading {
                level: HeadingLevel::H2,
                text,
            } => vec![Self::childless_block(BlockToCreate::Heading2 {
                heading_2: text.into_iter().collect(),
            })],
            Tag::Heading {
                level: HeadingLevel::H3,
                text,
            } => vec![Self::childless_block(BlockToCreate::Heading3 {
                heading_3: text.into_iter().collect(),
            })],
            Tag::Paragraph(Paragraph { text }) => {
                vec![Self::childless_block(BlockToCreate::Paragraph {
                    paragraph: text.into_iter().collect(),
                })]
            }
            Tag::OrderedList { items } => items
                .into_iter()
                .map(|item| BlockWithChildrenToCreate {
                    block: BlockToCreate::NumberedListItem {
                        numbered_list_item: notion::models::TextAndChildren {
                            rich_text: item.text.iter().map(Into::into).collect(),
                            children: Some(Vec::new()),
                            color: notion::models::text::TextColor::Default,
                        },
                    },
                    children: item
                        .children
                        .into_iter()
                        .flat_map(Self::from_markdown_tag)
                        .collect(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct ChildrenToCreate {
    children: Vec<BlockToCreate>,
}

#[async_recursion]
pub async fn get_all_block_children(
    notion_api: &notion::NotionApi,
    block_id: &BlockId,
) -> Result<Vec<BlockWithChildren>, notion::Error> {
    let children = notion_api.get_block_children(block_id).await?;

    if children.has_more {
        // TODO: handle pagination
        todo!("pagination when retrieving blocks")
    }

    join_all(children.results.into_iter().map(|child_block| async {
        let has_children = child_block
            .common()
            .map_or(false, |common| common.has_children);

        if has_children {
            get_all_block_children(notion_api, child_block.as_id())
                .await
                .map(|children| BlockWithChildren {
                    block: child_block,
                    children,
                })
        } else {
            Ok(BlockWithChildren {
                block: child_block,
                children: Vec::new(),
            })
        }
    }))
    .await
    .into_iter()
    .collect()
}

#[derive(Error, Debug)]
pub enum ErasePageError {
    #[error("could not fetch page content")]
    FetchingPageContent(#[from] Box<notion::Error>),

    #[error("deleting block failed")]
    DeleteBlockFailed {
        error: reqwest::Error,
        block_id: BlockId,
    },
}

pub async fn erase_page(
    notion_api: &notion::NotionApi,
    client: &NotionClient,
    page_id: PageId,
) -> Result<(), ErasePageError> {
    let block_id: BlockId = page_id.into();
    let blocks = notion_api
        .get_block_children(block_id)
        .await
        .map_err(|error| ErasePageError::FetchingPageContent(Box::new(error)))?
        .results;

    join_all(blocks.into_iter().map(|block| async move {
        client
            .delete_block(block.as_id().clone())
            .await
            .map_err(|error| ErasePageError::DeleteBlockFailed {
                error,
                block_id: block.as_id().clone(),
            })
    }))
    .await
    .into_iter()
    // NOTE: only expose the first error. Discards information about other errors
    .collect::<Result<Vec<()>, _>>()
    .map(|_| ())
}
