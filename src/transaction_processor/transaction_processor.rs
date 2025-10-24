use std::{collections::HashMap, sync::RwLock};
use std::sync::Arc;

use solana_sdk::{
    pubkey::Pubkey, 
    // system_program,
    transaction::Transaction
};

use crate::state_manager::state_manager::{L2Account, StateManager};


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct L2Transaction {
    pub signature: String,
    pub from: Pubkey,
    pub to: Option<Pubkey>,
    pub lamports: u64,
    pub instruction_data: Vec<u8>,
    pub nonce: u64
}

pub struct TransactionProcessor {
    state_manager: Arc<StateManager>,
    nonce_tracker: Arc<RwLock<HashMap<Pubkey, u64>>>
}

impl TransactionProcessor {
    pub fn new(state_manager: Arc<StateManager>) -> Self {
        Self {
            state_manager,
            nonce_tracker: Arc::new(RwLock::new(HashMap::new()))
        }
    }

    pub async fn process_transaction(&self, tx: &Transaction) -> anyhow::Result<String> {
        // validate tx
        self.validate_transaction(tx).await?;

        // convert to l2
        let l2_tx = self.convert_to_l2_transaction(tx)?;

        // exec tx
        self.execute_l2_transaction(&l2_tx).await?;
        Ok(l2_tx.signature)
    }

    async fn validate_transaction(&self, tx: &Transaction) -> anyhow::Result<()> {
        if !tx.verify().is_ok() {
            anyhow::bail!("Invalid transaction signatures");
        }

        // check nonce
        if let Some(fee_payer) = tx.message.account_keys.get(0) {
            let current_nonce = self.nonce_tracker
                .read()
                .unwrap()
                .get(fee_payer)
                .copied()
                .unwrap_or(0);

            // TODO: extract nonce from transaction
            // For now, just increment
        }

        Ok(())
    }

    fn convert_to_l2_transaction(&self, tx: &Transaction) -> anyhow::Result<L2Transaction> {
        let signature = tx.signatures.get(0)
            .ok_or_else(|| anyhow::anyhow!("No signature found"))?;

        let fee_payer = tx.message.account_keys.get(0)
            .ok_or_else(||anyhow::anyhow!("No fee payer found"))?;

        // Handle different instruction types
        if let Some(instruction) = tx.message.instructions.get(0) {
            if instruction.program_id_index == 0 { // System program
                let instruction_data = &instruction.data;

                // decode system instruction
                if instruction_data.len() >=4 {
                    let instruction_type = u32::from_le_bytes([
                        instruction_data[0],instruction_data[1],instruction_data[2],instruction_data[3]
                    ]);

                    match instruction_type {
                        2 => { // Transfer
                            let lamports = u64::from_le_bytes([
                                instruction_data[4], instruction_data[5],
                                instruction_data[6], instruction_data[7],
                                instruction_data[8], instruction_data[9],
                                instruction_data[10], instruction_data[11]
                            ]);

                            let to_pubkey = if instruction.accounts.len() > 1 {
                                Some(*tx.message.account_keys.get(instruction.accounts[1] as usize).unwrap())
                            } else {
                                None
                            };

                            return Ok(L2Transaction {
                                signature: signature.to_string(),
                                from: *fee_payer,
                                to: to_pubkey,
                                lamports,
                                instruction_data: instruction_data.to_vec(),
                                nonce: 0 // TODO: increment
                            })
                        }
                         _ => {}
                    }
                }
            }
        }

        // Default
        Ok(L2Transaction {
            signature: signature.to_string(),
            from: *fee_payer,
            to: None,
            lamports: 0,
            instruction_data: vec![],
            nonce: 0
        })
    }

    async fn execute_l2_transaction(&self, tx: &L2Transaction) -> anyhow::Result<()> {
        match tx.to {
            Some(to_pubkey) => {
                self.transfer_lamports(&tx.from, &to_pubkey, tx.lamports).await?;
            },
            None => {
                // Handle other tx types
                println!("Non-transfer transaction: {:?}", tx);
            }
        }

        // update nonce
        let mut nonces = self.nonce_tracker.write().unwrap();
        let current_nonce = nonces.get(&tx.from).copied().unwrap_or(0);
        nonces.insert(tx.from, current_nonce + 1);

        Ok(())
    }

    async fn transfer_lamports(&self, from: &Pubkey, to: &Pubkey, amount: u64) -> anyhow::Result<()>{
        let system_program_id = Pubkey::from_str_const("11111111111111111111111111111111");

        let mut from_account = self.state_manager.get_account(from).await
            .unwrap_or_else(|| L2Account {
                lamports: 0,
                data: vec![],
                owner: system_program_id,
                executable: false,
                rent_epoch: 0
            });

        //  check sufficent balance
        if from_account.lamports < amount {
            anyhow::bail!("Insufficient funds");
        }

        let mut to_account = self.state_manager.get_account(to).await
            .unwrap_or_else(|| L2Account {
                lamports: 0,
                data: vec![],
                owner: system_program_id,
                executable: false,
                rent_epoch: 0,
            });

        // update balances
        from_account.lamports -= amount;
        to_account.lamports += amount;

        // save state
        self.state_manager.update_account(from, from_account).await?;
        self.state_manager.update_account(to, to_account).await?;

        Ok(())
    }
    
}
