use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use solana_sdk::transaction::Transaction;

use crate::state_manager::state_manager::StateManager;

#[derive(Debug, Clone)]
pub struct Sequencer {
    pending_txs: Arc<RwLock<Vec<Transaction>>>,
    batch_sender: mpsc::Sender<Vec<Transaction>>,
}

impl Sequencer {
    pub fn new(state_manager: Arc<StateManager>) -> (Self, mpsc::Receiver<Vec<Transaction>>) {
        let (batch_sender, batch_receiver) = mpsc::channel(100);

        let sequencer = Self {
            pending_txs: Arc::new(RwLock::new(Vec::new())),
            batch_sender,
        };

        (sequencer, batch_receiver)
    }

    pub async fn add_transaction(&self, tx: Transaction) {
        let mut pending = self.pending_txs.write().await;
        pending.push(tx);
    }

    pub async fn start_batching(&self) {
        let mut interval = interval(tokio::time::Duration::from_secs(2));
        loop {
            interval.tick().await;
            self.create_batch().await;
        }
    }

    async fn create_batch(&self) {
        let mut pending = self.pending_txs.write().await;

        if pending.is_empty() {
            return;
        }

        let batch_size = std::cmp::min(pending.len(), 100);
        let batch: Vec<Transaction> = pending.drain(..batch_size).collect();

        println!("Creating batch with {} transactions", batch.len());

        if let Err(e) = self.batch_sender.send(batch).await {
            eprintln!("Failed to send batch: {}", e);
        }
    }
}
