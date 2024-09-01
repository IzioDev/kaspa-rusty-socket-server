use kaspa_hashes::Hash;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerTransaction {
    pub tx_id: Hash,
    pub outputs: Vec<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerBlockAdded {
    #[serde(rename = "block_hash")]
    pub block_hash: Hash,
    pub blue_score: u64,
    pub difficulty: f64,
    pub timestamp: u64,
    pub tx_count: i64,
    pub txs: Vec<ExplorerTransaction>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerBlueScoreChanged {
    pub blue_score: u64,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ExplorerEvent {
    BlockAdded(ExplorerBlockAdded),
    BlueScoreChanged(ExplorerBlueScoreChanged),
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ExplorerMessage {
    EventType(String),
    ExplorerEvent(ExplorerEvent),
}
