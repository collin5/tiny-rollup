use std::{collections::HashMap, sync::{Arc, RwLock}};

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Account {
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64
}

#[derive(Debug, Clone)]
pub struct StateManager {
    accounts: Arc<RwLock<HashMap<Pubkey, L2Account>>>,
    db: Arc<rocksdb::DB>,
    state_root: Arc<RwLock<[u8; 32]>>
}

impl StateManager {
    pub fn new(db_path: &str) -> anyhow::Result<Self> {
        let db = rocksdb::DB::open_default(db_path)?;

        Ok(Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
            db: Arc::new(db),
            state_root: Arc::new(RwLock::new([0u8; 32]))
        })
    }

    pub async fn get_account(&self, pubkey: &Pubkey) -> Option<L2Account> {
        // First we check memory
        if let Some(account) = self.accounts.read().unwrap().get(pubkey) {
            return Some(account.clone());
        }

        // Then check persistent storage
        if let Ok(Some(data)) = self.db.get(pubkey.to_bytes()) {
            if let Ok(account) = bincode::deserialize::<L2Account>(&data) {
                // cache result in memory
                self.accounts.write().unwrap().insert(*pubkey, account.clone());
                return Some(account);
            }
        }

        None
    }

    pub async fn update_account(&self, pubkey: &Pubkey, account: L2Account) -> anyhow::Result<()> {
        // Update memory
        self.accounts.write().unwrap().insert(*pubkey, account.clone());

        // persist to storage
        let serialized = bincode::serialize(&account)?;
        self.db.put(pubkey.to_bytes(), serialized)?;

        // Update state root
        self.update_state_root().await?;

        Ok(())
    }

    async fn update_state_root(&self) -> anyhow::Result<()> {
        // TODO: use proper Merkle tree
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let accounts = self.accounts.read().unwrap();
        let mut hasher = DefaultHasher::new();

        for (pubkey, account) in accounts.iter() {
            pubkey.hash(&mut hasher);
            account.lamports.hash(&mut hasher);
            account.data.hash(&mut hasher);
        }

        let hash = hasher.finish();
        let mut state_root = [0u8; 32];
        state_root[..8].copy_from_slice(&hash.to_le_bytes());
        *self.state_root.write().unwrap() = state_root;
        Ok(())
    }

    pub fn get_state_root(&self) -> [u8; 32] {
        *self.state_root.read().unwrap()
    }
}
