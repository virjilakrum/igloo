use std::sync::Arc;

use super::{head::L1HeadImpl, L1Hash, L1Height, L1Timestamp};
use crate::l2::tx::L2Transaction;
use rollups_interface::l1::{Epoch, PayloadAttribute};

pub struct EpochInfo {
    hash: L1Hash,
    height: L1Height,
    timestamp: L1Timestamp,
}

impl Epoch for EpochInfo {
    type Hash = L1Hash;
    type BlockHeight = L1Height;
    type Timestamp = L1Timestamp;

    fn block_hash(&self) -> Self::Hash {
        self.hash
    }

    fn block_height(&self) -> Self::BlockHeight {
        self.height
    }

    fn timestamp(&self) -> Self::Timestamp {
        self.timestamp
    }
}

impl TryFrom<L1HeadImpl> for EpochInfo {
    type Error = anyhow::Error;

    fn try_from(value: L1HeadImpl) -> Result<Self, Self::Error> {
        Ok(Self {
            hash: value.hash,
            height: value.height,
            timestamp: value.timestamp,
        })
    }
}

pub struct PayloadAttributeImpl {
    pub transactions: Arc<Vec<L2Transaction>>,
    pub epoch: EpochInfo,
}

impl PayloadAttribute for PayloadAttributeImpl {
    type Transaction = L2Transaction;
    type Epoch = EpochInfo;

    fn transactions(&self) -> std::sync::Arc<Vec<Self::Transaction>> {
        self.transactions.clone()
    }

    fn epoch_info(&self) -> &Self::Epoch {
        &self.epoch
    }
}