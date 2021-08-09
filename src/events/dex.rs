use tendermint::block::Height;
use serde_derive::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OracleRequest {
    height: Height,
    hash: String,
    sender: String,

    // some packet data
    ibc_packet_sec: u64,
    oracle_script: u64,
    ask_count: u32,
    min_count: u32,
}
