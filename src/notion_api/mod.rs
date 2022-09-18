use notion::models::{Block, BlockCommon};

pub mod client;

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
