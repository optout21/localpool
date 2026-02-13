use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents a JSON-RPC command with its parameters
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcCommand {
    pub method: String,
    pub params: Value,
    pub id: Option<Value>,
    pub headers: Option<Vec<(String, String)>>,
}
