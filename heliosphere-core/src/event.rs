#![allow(missing_docs)]

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// Event item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventData {
    /// block number
    pub block_number: u64,
    /// block timestamp
    pub block_timestamp: u64,
    /// event index
    pub event_index: u64,
    /// event name
    pub event_name: String,
    /// transaction id
    pub transaction_id: String,
    pub event: String,
    pub result: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventMeta {
    pub at: u64,
    pub page_size: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventsResult {
    pub data: Vec<EventData>,
    pub success: bool,
    pub error: Option<String>,
    #[serde(rename(deserialize = "statusCode"))]
    pub status_code: Option<u8>,
    pub meta: EventMeta,
}
