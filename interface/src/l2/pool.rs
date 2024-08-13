use super::Transaction;

pub trait BatchSettings {
    fn max_size(&self) -> usize;
}

pub trait TransactionPool {
    type TxIn: Transaction;
    type TxOut: Transaction;
    type Settings: BatchSettings;

    fn insert(&mut self, tx: Self::TxIn);

    fn next_batch(&mut self, settings: Self::Settings) -> Vec<Self::TxOut>;
}