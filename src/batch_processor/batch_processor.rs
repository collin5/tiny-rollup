use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig
};
use solana_commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::{
    pubkey::Pubkey, 
    signature::{Keypair, Signer},
    transaction::Transaction
};
use tokio::sync::mpsc;

pub struct BatchProcessor {
    solana_client: RpcClient,
    rollup_program_id: Pubkey,
    authority: Keypair
}

impl BatchProcessor {
    pub fn new(solana_rpc_url: String) -> Self {
        Self {
            solana_client: RpcClient::new_with_commitment(solana_rpc_url, CommitmentConfig::confirmed()),
            rollup_program_id: Pubkey::new_unique(), // Rollup program id
            authority: Keypair::new(), // Load from config
        }
    }

    pub async fn process_batches(&self, mut batch_reciever: mpsc::Receiver<Vec<Transaction>>) {
        while let Some(batch) = batch_reciever.recv().await {
            if let Err(e) = self.submit_batch_to_l1(batch).await {
                eprint!("Failed to submit batch to L1: {}", e)
            }
        }
    }

    async fn submit_batch_to_l1(&self, batch: Vec<Transaction>) -> anyhow::Result<()> {
        let batch_data = self.compress_batch(&batch)?;

        let instruction = solana_sdk::instruction::Instruction::new_with_bytes(
            self.rollup_program_id,
            &batch_data,
            vec![] // Account metas for rollup program
        );

        let recent_blockhash = self.solana_client.get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.authority.pubkey()),
            &[&self.authority],
            recent_blockhash
        );

        let config = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            ..Default::default()
        };

        let signature = self.solana_client.send_transaction_with_config(&tx, config).await?;

        println!("Batch submitted to L1: {}", signature);

        Ok(())

    }

    fn compress_batch(&self, batch: &[Transaction]) -> anyhow::Result<Vec<u8>> {
        let serialzed = bincode::serialize(batch)?;

        // TODO: Use LZ4 or similar
        Ok(serialzed)
    }
}
