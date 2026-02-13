use std::u32;

use crate::local_tx::LocalTx;
use crate::rpc_command::RpcCommand;

#[derive(Debug, Clone)]
pub struct LocalTxs {
    txs: Vec<LocalTx>,
}

impl LocalTxs {
    pub fn new() -> Self {
        Self { txs: Vec::new() }
    }

    pub fn add(&mut self, txhex: String, target_time: u32, submitter_rpc: RpcCommand) {
        self.txs
            .push(LocalTx::new(txhex, target_time, submitter_rpc));
    }

    pub fn len(&self) -> usize {
        self.txs.len()
    }

    pub fn get_earliest(&self) -> Option<LocalTx> {
        let mut min_time = u32::MAX;
        let mut min_tx: Option<LocalTx> = None;
        for tx in &self.txs {
            if tx.target_time() < min_time {
                min_time = tx.target_time();
                min_tx = Some(tx.clone());
            }
        }
        min_tx
    }

    /// Removes and returns the earliest transaction
    pub fn remove_earliest(&mut self) -> Option<LocalTx> {
        if self.txs.is_empty() {
            return None;
        }

        let mut min_time = u32::MAX;
        let mut min_idx = 0;

        for (idx, tx) in self.txs.iter().enumerate() {
            if tx.target_time() < min_time {
                min_time = tx.target_time();
                min_idx = idx;
            }
        }

        Some(self.txs.remove(min_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Helper function to create a test RpcCommand
    fn create_test_rpc_command(method: &str, id: i32) -> RpcCommand {
        RpcCommand {
            method: method.to_string(),
            params: json!(["param1", "param2"]),
            id: Some(json!(id)),
            headers: None,
        }
    }

    #[test]
    fn test_new_creates_empty_local_txs() {
        let local_txs = LocalTxs::new();
        assert_eq!(local_txs.len(), 0);
    }

    #[test]
    fn test_add_single_transaction() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should go forward")
            .as_secs() as u32;
        let mut local_txs = LocalTxs::new();
        let rpc = create_test_rpc_command("sendrawtransaction", 1);

        local_txs.add("deadbeef".to_string(), now + 3600, rpc);

        assert_eq!(local_txs.len(), 1);
    }

    #[test]
    fn test_add_multiple_transactions() {
        let mut local_txs = LocalTxs::new();

        for i in 0..5 {
            let rpc = create_test_rpc_command("sendrawtransaction", i);
            local_txs.add(format!("tx{}", i), 0, rpc);
        }

        assert_eq!(local_txs.len(), 5);
    }

    #[test]
    fn test_len_returns_correct_count() {
        let mut local_txs = LocalTxs::new();
        assert_eq!(local_txs.len(), 0);

        let rpc1 = create_test_rpc_command("sendrawtransaction", 1);
        local_txs.add("tx1".to_string(), 0, rpc1);
        assert_eq!(local_txs.len(), 1);

        let rpc2 = create_test_rpc_command("sendrawtransaction", 2);
        local_txs.add("tx2".to_string(), 0, rpc2);
        assert_eq!(local_txs.len(), 2);

        let rpc3 = create_test_rpc_command("sendrawtransaction", 3);
        local_txs.add("tx3".to_string(), 0, rpc3);
        assert_eq!(local_txs.len(), 3);
    }

    #[test]
    fn test_get_earliest_on_empty_returns_none() {
        let local_txs = LocalTxs::new();
        assert!(local_txs.get_earliest().is_none());
    }

    #[test]
    fn test_get_earliest_with_single_transaction() {
        let now = 1780000000u32;
        let mut local_txs = LocalTxs::new();
        let rpc = create_test_rpc_command("sendrawtransaction", 1);

        local_txs.add("single_tx".to_string(), now + 3600, rpc);

        let earliest = local_txs.get_earliest();
        assert!(earliest.is_some());

        let tx = earliest.unwrap();
        assert_eq!(tx.target_time(), 1780003600); // LocalTx::new sets submitted_time to 0
    }

    #[test]
    fn test_get_earliest_with_multiple_transactions() {
        let now = 1780000000u32;
        let mut local_txs = LocalTxs::new();

        // Add multiple transactions (all will have submitted_time = 0 due to LocalTx::new)
        for i in 0..3 {
            let rpc = create_test_rpc_command("sendrawtransaction", i);
            local_txs.add(
                format!("tx{}", i),
                (now as i32 + 3600 - 120 * i) as u32,
                rpc,
            );
        }

        let earliest = local_txs.get_earliest();
        assert!(earliest.is_some());

        // Since all have submitted_time = 0, it should return the first one found
        let tx = earliest.unwrap();
        assert_eq!(tx.target_time(), 1780003360);
    }

    #[test]
    fn test_add_with_empty_txhex() {
        let mut local_txs = LocalTxs::new();
        let rpc = create_test_rpc_command("sendrawtransaction", 1);

        local_txs.add("".to_string(), 0, rpc);

        assert_eq!(local_txs.len(), 1);
    }

    #[test]
    fn test_add_with_long_txhex() {
        let mut local_txs = LocalTxs::new();
        let rpc = create_test_rpc_command("sendrawtransaction", 1);
        let long_hex = "a".repeat(10000);

        local_txs.add(long_hex, 0, rpc);

        assert_eq!(local_txs.len(), 1);
    }

    #[test]
    fn test_clone_local_txs() {
        let mut local_txs = LocalTxs::new();
        let rpc = create_test_rpc_command("sendrawtransaction", 1);
        local_txs.add("tx1".to_string(), 0, rpc);

        let cloned = local_txs.clone();

        assert_eq!(cloned.len(), local_txs.len());
        assert_eq!(cloned.len(), 1);
    }

    #[test]
    fn test_add_with_different_rpc_commands() {
        let mut local_txs = LocalTxs::new();

        let rpc1 = RpcCommand {
            method: "sendrawtransaction".to_string(),
            params: json!(["tx1"]),
            id: Some(json!(1)),
            headers: None,
        };

        let rpc2 = RpcCommand {
            method: "submitblock".to_string(),
            params: json!({"block": "data"}),
            id: Some(json!("string_id")),
            headers: Some(vec![(
                "Authorization".to_string(),
                "Bearer token".to_string(),
            )]),
        };

        local_txs.add("tx1".to_string(), 0, rpc1);
        local_txs.add("tx2".to_string(), 0, rpc2);

        assert_eq!(local_txs.len(), 2);
    }

    #[test]
    fn test_multiple_operations_sequence() {
        let mut local_txs = LocalTxs::new();

        // Start empty
        assert_eq!(local_txs.len(), 0);
        assert!(local_txs.get_earliest().is_none());

        // Add first transaction
        let rpc1 = create_test_rpc_command("sendrawtransaction", 1);
        local_txs.add("tx1".to_string(), 0, rpc1);
        assert_eq!(local_txs.len(), 1);
        assert!(local_txs.get_earliest().is_some());

        // Add second transaction
        let rpc2 = create_test_rpc_command("sendrawtransaction", 2);
        local_txs.add("tx2".to_string(), 0, rpc2);
        assert_eq!(local_txs.len(), 2);

        // Add third transaction
        let rpc3 = create_test_rpc_command("sendrawtransaction", 3);
        local_txs.add("tx3".to_string(), 0, rpc3);
        assert_eq!(local_txs.len(), 3);

        // Verify earliest still works
        assert!(local_txs.get_earliest().is_some());
    }

    #[test]
    fn test_remove_earliest_on_empty_returns_none() {
        let mut local_txs = LocalTxs::new();
        assert!(local_txs.remove_earliest().is_none());
    }

    #[test]
    fn test_remove_earliest_with_single_transaction() {
        let mut local_txs = LocalTxs::new();
        let rpc = create_test_rpc_command("sendrawtransaction", 1);
        local_txs.add("tx1".to_string(), 100, rpc);

        assert_eq!(local_txs.len(), 1);

        let removed = local_txs.remove_earliest();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().target_time(), 100);
        assert_eq!(local_txs.len(), 0);
    }

    #[test]
    fn test_remove_earliest_with_multiple_transactions() {
        let mut local_txs = LocalTxs::new();

        let rpc1 = create_test_rpc_command("sendrawtransaction", 1);
        local_txs.add("tx1".to_string(), 300, rpc1);

        let rpc2 = create_test_rpc_command("sendrawtransaction", 2);
        local_txs.add("tx2".to_string(), 100, rpc2);

        let rpc3 = create_test_rpc_command("sendrawtransaction", 3);
        local_txs.add("tx3".to_string(), 200, rpc3);

        assert_eq!(local_txs.len(), 3);

        // Remove earliest (should be tx2 with target_time 100)
        let removed = local_txs.remove_earliest();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().target_time(), 100);
        assert_eq!(local_txs.len(), 2);

        // Remove next earliest (should be tx3 with target_time 200)
        let removed = local_txs.remove_earliest();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().target_time(), 200);
        assert_eq!(local_txs.len(), 1);

        // Remove last (should be tx1 with target_time 300)
        let removed = local_txs.remove_earliest();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().target_time(), 300);
        assert_eq!(local_txs.len(), 0);

        // Try to remove from empty
        assert!(local_txs.remove_earliest().is_none());
    }
}
