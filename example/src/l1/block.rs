use std::sync::Arc;

use rollups_interface::l1::L1BlockInfo;

use crate::l2::tx::L2Transaction;

use super::{attribute::PayloadAttributeImpl, batch, head::L1HeadImpl, tx};

pub struct L1BlockInfoImpl {
    deposit_txs: Vec<tx::DepositTx>,
    batch: batch::Batch,
    l1_head: L1HeadImpl,
}

impl L1BlockInfo<PayloadAttributeImpl> for L1BlockInfoImpl {
    type DepositTx = tx::DepositTx;
    type Batch = batch::Batch;
    type L1Head = L1HeadImpl;

    fn deposit_transactions(&self) -> &[Self::DepositTx] {
        self.deposit_txs.as_slice()
    }

    fn batch_info(&self) -> &Self::Batch {
        &self.batch
    }

    fn l1_head(&self) -> &Self::L1Head {
        &self.l1_head
    }
}

impl TryInto<PayloadAttributeImpl> for L1BlockInfoImpl {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<PayloadAttributeImpl, Self::Error> {
        let deposit_tx = self
            .deposit_txs
            .into_iter()
            .map(|tx| tx.try_into())
            .collect::<anyhow::Result<Vec<L2Transaction>>>()?;
        let epoch = self.l1_head.try_into()?;
        // TODO: derive batch from `self.batch` if needed later

        Ok(PayloadAttributeImpl {
            transactions: Arc::new(deposit_tx),
            epoch,
        })
    }
}