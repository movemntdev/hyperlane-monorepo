use std::{num::NonZeroU64, ops::RangeInclusive, str::FromStr};

use async_trait::async_trait;
use derive_new::new;
use hyperlane_core::{
    accumulator::incremental::IncrementalMerkle, to_hex, ChainCommunicationError, ChainResult, Checkpoint, Indexer, LogMeta, MerkleTreeHook, MerkleTreeInsertion, SequenceIndexer, H256
};
use move_core_types::{
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    identifier::Identifier,
    language_storage::StructTag,
};
use sui_sdk::json::SuiJsonValue;
use tracing::instrument;

use crate::{move_view_call, AddressFormatter, SuiMailbox, TryIntoPrimitive};

#[async_trait]
impl MerkleTreeHook for SuiMailbox {
    #[instrument(err, ret, skip(self))]
    async fn tree(&self, _lag: Option<NonZeroU64>) -> ChainResult<IncrementalMerkle> {
        let view_response = move_view_call(
            &self.sui_client,
            &self.packages_address,
            self.packages_address,
            "mailbox".to_string(),
            "outbox_get_tree".to_string(),
            vec![],
            vec![],
        )
        .await?;
        let (bytes, type_tag) = &view_response[0].return_values[0];

        //construct the MoveStructLayout
        let branch_field = MoveFieldLayout::new(
            Identifier::from_str("branch").unwrap(),
            MoveTypeLayout::Vector(Box::new(MoveTypeLayout::Vector(Box::new(
                MoveTypeLayout::U8,
            )))),
        );
        let count_field =
            MoveFieldLayout::new(Identifier::from_str("count").unwrap(), MoveTypeLayout::U64);
        let tree_move_struct = MoveStructLayout::new(
            StructTag::from_str("MerkleTree").unwrap(),
            vec![branch_field, count_field],
        );
        let tree =
            SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::Struct(tree_move_struct)), bytes)
                .expect("Failed to deserialize tree");

        Ok(tree
            .try_into_merkle_tree()
            .expect("Failed to convert to IncrementalMerkle"))
    }

    #[instrument(err, ret, skip(self))]
    async fn latest_checkpoint(&self, lag: Option<NonZeroU64>) -> ChainResult<Checkpoint> {
        let tree = self.tree(lag).await?;

        let root = tree.root();
        let count: u32 = tree
            .count()
            .try_into()
            .map_err(ChainCommunicationError::from_other)?;
        let index = count.checked_sub(1).ok_or_else(|| {
            ChainCommunicationError::from_contract_error_str(
                "Outbux is empty, cannot compute checkpoint",
            )
        })?;

        let checkpoint = Checkpoint {
            merkle_tree_hook_address: H256::from_str(&to_hex(&self.packages_address.to_vec(), true))
                .expect("Failed to convert to H256"),
            mailbox_domain: self.domain.id(),
            root,
            index,
        };
        Ok(checkpoint)
    }

    #[instrument(err, ret, skip(self))]
    async fn count(&self, _maybe_lag: Option<NonZeroU64>) -> ChainResult<u32> {
        let tree = self.tree(_maybe_lag).await?;
        tree.count()
            .try_into()
            .map_err(ChainCommunicationError::from_other)
    }
}

/// Struct that retrieves event data for  a Sui merkle tree hook module.
#[derive(Debug, new)]
pub struct SuiMerkleTreeHookIndexer {}

#[async_trait]
impl Indexer<MerkleTreeInsertion> for SuiMerkleTreeHookIndexer {
    async fn fetch_logs(
        &self,
        _range: RangeInclusive<u32>,
    ) -> ChainResult<Vec<(MerkleTreeInsertion, LogMeta)>> {
        // Not iplemented for Sui or Aptos
        Ok(vec![])
    }

    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        // Not iplemented for Sui or Aptos
        Ok(0)
    }
}

#[async_trait]
impl SequenceIndexer<MerkleTreeInsertion> for SuiMerkleTreeHookIndexer {
    async fn sequence_and_tip(&self) -> ChainResult<(Option<u32>, u32)> {
        Ok((None, 0))
    }
}
