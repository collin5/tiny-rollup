use std::sync::Arc;

use jsonrpsee::{
    core::{RpcResult, async_trait},
    proc_macros::rpc,
    types::{ErrorObjectOwned, ErrorObject},
};
use serde_json::Value;
use solana_sdk::{pubkey::Pubkey, transaction::Transaction};

use crate::{
    sequencer::sequencer::Sequencer, 
    state_manager::state_manager::StateManager, 
    transaction_processor::transaction_processor::TransactionProcessor
};

#[rpc(server)]
pub trait RollupRpc {
    #[method(name = "getAccountInfo")]
    async fn get_account_info(
        &self,
        pubkey: String,
        config: Option<Value>
    ) -> RpcResult<Option<Value>>;

    #[method(name = "getBalance")]
    async fn get_balance(&self, pubkey: String, config: Option<Value>) -> RpcResult<u64>;
    
    #[method(name = "sendTransaction")]
    async fn send_transaction(&self, transaction: String, config: Option<Value>) -> RpcResult<String>;
    
    #[method(name = "getLatestBlockhash")]
    async fn get_latest_blockhash(&self, config: Option<Value>) -> RpcResult<Value>;
    
    #[method(name = "simulateTransaction")]
    async fn simulate_transaction(&self, transaction: String, config: Option<Value>) -> RpcResult<Value>;
    
    #[method(name = "getTransaction")]
    async fn get_transaction(&self, signature: String, config: Option<Value>) -> RpcResult<Option<Value>>;
}

pub struct RollupRpcImpl {
    state_manager: Arc<StateManager>,
    transaction_processor: Arc<TransactionProcessor>,
    sequencer: Arc<Sequencer>,
}

impl RollupRpcImpl {
    pub fn new(
        state_manager: Arc<StateManager>,
        transaction_processor: Arc<TransactionProcessor>,
        sequencer: Arc<Sequencer>
    ) -> Self {
        Self {
            state_manager,
            transaction_processor,
            sequencer,
        }
    }
}

#[async_trait]
impl RollupRpcServer for RollupRpcImpl {
    async fn get_account_info(
        &self,
        pubkey: String,
        _config: Option<Value>
    ) -> RpcResult<Option<Value>> {
        let pubkey = pubkey.parse::<Pubkey>()
            .map_err(|e| ErrorObjectOwned::owned(-32602, "Invalid pubkey", Some(e.to_string())))?;

        match self.state_manager.get_account(&pubkey).await {
            Some(account) => {
                let account_info = serde_json::json!({
                    "data": [bs58::encode(&account.data).into_string(), "base58"],
                    "executable": account.executable,
                    "lamports": account.lamports,
                    "owner": account.owner.to_string(),
                    "rentEpoch": account.rent_epoch
                });

                Ok(Some(serde_json::json!({
                    "value": account_info
                })))
            }
            None => Ok(Some(serde_json::json!({"value": null})))
        }
    }

    async fn get_balance(&self, pubkey: String, _config: Option<Value>) -> RpcResult<u64> {
        let pubkey = pubkey.parse::<Pubkey>()
            .map_err(|e| ErrorObjectOwned::owned(-32602, "Invalid pubkey", Some(e.to_string())))?;

        let account = self.state_manager.get_account(&pubkey).await;
        Ok(account.map(|a| a.lamports).unwrap_or(0))
    }

    async fn send_transaction(&self, transaction: String, _config: Option<Value>) -> RpcResult<String> {
        let tx_bytes = bs58::decode(transaction)
            .into_vec()
            .map_err(|e| ErrorObjectOwned::owned(-32602, "Invalid transaction encoding", Some(e.to_string())))?;

        let tx: Transaction = bincode::deserialize(&tx_bytes)
            .map_err(|e| ErrorObjectOwned::owned(-32602, "Invalid transaction format", Some(e.to_string())))?;

        let signature = self.transaction_processor.process_transaction(&tx).await
            .map_err(|e| ErrorObjectOwned::owned(-32000, "Transaction processing failed", Some(e.to_string())))?;

        // Add to sequencer queue
        self.sequencer.add_transaction(tx).await;

        Ok(signature)
    }

    async fn get_latest_blockhash(&self, _config: Option<Value>) -> RpcResult<Value> {
        let blockhash = format!("{}1111111111111111111111111111", hex::encode(&self.state_manager.get_state_root()[..8]));

        Ok(serde_json::json!({
            "value": {
                "blockhash": blockhash,
                "lastValidBlockHeight": 999999999u64
            }
        }))
    }

    async fn simulate_transaction(&self, transaction: String, _config: Option<Value>) -> RpcResult<Value> {
        let tx_bytes = bs58::decode(transaction)
            .into_vec()
            .map_err(|e| ErrorObjectOwned::owned(-32602, "Invalid transaction encoding", Some(e.to_string())))?;

        let _tx: Transaction = bincode::deserialize(&tx_bytes)
            .map_err(|e| ErrorObjectOwned::owned(-32602, "Invalid transaction format", Some(e.to_string())))?;

        Ok(serde_json::json!({
            "value": {
                "err": null,
                "logs": [],
                "accounts": null,
                "unitsConsumed": 1000
            }
        }))
    }

    async fn get_transaction(&self, _signature: String, _config: Option<Value>) -> RpcResult<Option<Value>> {
        Ok(None)
    }
}
