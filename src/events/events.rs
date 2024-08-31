use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ExplorerTransaction {
    pub txId: String,
    pub outputs: Vec<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct ExplorerBlockAdded {
    pub block_hash: String,
    pub blueScore: String,
    pub difficulty: i64,
    pub timestamp: String,
    pub txCount: i64,
    pub txs: Vec<ExplorerTransaction>,
}
