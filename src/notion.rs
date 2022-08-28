use async_recursion::async_recursion;
use futures::future::join_all;
use notion::{
    ids::{AsIdentifier, BlockId},
    models::{Block, BlockCommon},
    NotionApi,
};

#[async_recursion]
pub async fn get_all_block_children(
    notion_api: &NotionApi,
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
            get_all_block_children(&notion_api, child_block.as_id())
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

#[derive(Debug)]
pub struct BlockWithChildren {
    pub block: Block,
    pub children: Vec<BlockWithChildren>,
}

trait GetCommon {
    fn common(&self) -> Option<&BlockCommon>;
}

impl GetCommon for Block {
    fn common(&self) -> Option<&BlockCommon> {
        use Block::*;

        match self {
            NumberedListItem { common, .. } => Some(common),
            // TODO: add more block patterns
            _ => None,
        }
    }
}
