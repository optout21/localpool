use crate::rpc_command::RpcCommand;

use std::sync::{Arc, Mutex};

/// A test stub for handling JSON-RPC commands
///
/// CommandStore can accept JSON RPC commands, store them, log them,
/// and return them upon request for testing purposes.
#[derive(Debug, Clone)]
pub struct CommandStore {
    commands: Arc<Mutex<Vec<RpcCommand>>>,
    logging_enabled: bool,
}

impl CommandStore {
    /// Creates a new CommandStore with logging enabled by default
    pub fn new() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            logging_enabled: true,
        }
    }

    /// Creates a new CommandStore with logging disabled
    pub fn new_without_logging() -> Self {
        Self {
            commands: Arc::new(Mutex::new(Vec::new())),
            logging_enabled: false,
        }
    }

    /// Accepts and stores a JSON-RPC command
    ///
    /// # Arguments
    /// * `method` - The RPC method name
    /// * `params` - The parameters as a JSON value
    /// * `id` - Optional request ID
    pub fn accept_command(
        &self,
        method: impl Into<String>,
        params: serde_json::Value,
        id: Option<serde_json::Value>,
    ) {
        let command = RpcCommand {
            method: method.into(),
            params,
            id,
            headers: None,
        };

        if self.logging_enabled {
            println!("[CommandStore] Received command: {:?}", command);
        }

        let mut commands = self.commands.lock().unwrap();
        commands.push(command);
    }

    /// Accepts a JSON-RPC command from a JSON string
    ///
    /// # Arguments
    /// * `json_str` - A JSON string containing the RPC command
    ///
    /// # Returns
    /// * `Result<(), String>` - Ok if successful, Err with error message otherwise
    #[cfg(test)]
    pub fn accept_json(
        &self,
        json_str: &str,
        headers: Option<Vec<(String, String)>>,
    ) -> Result<(), String> {
        let mut command: RpcCommand =
            serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;
        command.headers = headers;
        if self.logging_enabled {
            println!("[CommandStore] Received JSON command: {:?}", command);
        }

        let mut commands = self.commands.lock().unwrap();
        commands.push(command);
        Ok(())
    }

    /// Returns all stored commands
    pub fn get_commands(&self) -> Vec<RpcCommand> {
        let commands = self.commands.lock().unwrap();
        commands.clone()
    }

    /// Returns the last command received, if any
    pub fn get_last_command(&self) -> Option<RpcCommand> {
        let commands = self.commands.lock().unwrap();
        commands.last().cloned()
    }

    /// Returns all commands with a specific method name
    pub fn get_commands_by_method(&self, method: &str) -> Vec<RpcCommand> {
        let commands = self.commands.lock().unwrap();
        commands
            .iter()
            .filter(|cmd| cmd.method == method)
            .cloned()
            .collect()
    }

    /// Returns the number of commands received
    pub fn command_count(&self) -> usize {
        let commands = self.commands.lock().unwrap();
        commands.len()
    }

    /// Clears all stored commands
    pub fn clear(&self) {
        let mut commands = self.commands.lock().unwrap();
        commands.clear();
        if self.logging_enabled {
            println!("[CommandStore] Cleared all commands");
        }
    }

    /// Enables or disables logging
    #[cfg(test)]
    pub fn set_logging(&mut self, enabled: bool) {
        self.logging_enabled = enabled;
    }

    /// Returns whether logging is enabled
    #[cfg(test)]
    pub fn is_logging_enabled(&self) -> bool {
        self.logging_enabled
    }
}

impl Default for CommandStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_accept_command() {
        let stub = CommandStore::new_without_logging();

        stub.accept_command("getblockcount", json!({}), Some(json!(1)));

        assert_eq!(stub.command_count(), 1);

        let commands = stub.get_commands();
        assert_eq!(commands[0].method, "getblockcount");
        assert_eq!(commands[0].id, Some(json!(1)));
    }

    #[test]
    fn test_accept_json() {
        let stub = CommandStore::new_without_logging();

        let json_cmd = r#"{"method":"getblock","params":{"hash":"abc123"},"id":2}"#;
        stub.accept_json(json_cmd, None).unwrap();

        assert_eq!(stub.command_count(), 1);

        let last = stub.get_last_command().unwrap();
        assert_eq!(last.method, "getblock");
        assert_eq!(last.params, json!({"hash": "abc123"}));
        assert_eq!(last.id, Some(json!(2)));
    }

    #[test]
    fn test_multiple_commands() {
        let stub = CommandStore::new_without_logging();

        stub.accept_command("method1", json!([1, 2, 3]), Some(json!(1)));
        stub.accept_command("method2", json!({"key": "value"}), Some(json!(2)));
        stub.accept_command("method1", json!([4, 5, 6]), Some(json!(3)));

        assert_eq!(stub.command_count(), 3);

        let method1_commands = stub.get_commands_by_method("method1");
        assert_eq!(method1_commands.len(), 2);
        assert_eq!(method1_commands[0].params, json!([1, 2, 3]));
        assert_eq!(method1_commands[1].params, json!([4, 5, 6]));
    }

    #[test]
    fn test_get_last_command() {
        let stub = CommandStore::new_without_logging();

        assert!(stub.get_last_command().is_none());

        stub.accept_command("first", json!({}), None);
        stub.accept_command("second", json!({}), None);

        let last = stub.get_last_command().unwrap();
        assert_eq!(last.method, "second");
    }

    #[test]
    fn test_clear() {
        let stub = CommandStore::new_without_logging();

        stub.accept_command("test", json!({}), None);
        assert_eq!(stub.command_count(), 1);

        stub.clear();
        assert_eq!(stub.command_count(), 0);
        assert!(stub.get_last_command().is_none());
    }

    #[test]
    fn test_get_commands_by_method() {
        let stub = CommandStore::new_without_logging();

        stub.accept_command("getblock", json!({"height": 100}), Some(json!(1)));
        stub.accept_command("gettransaction", json!({"txid": "abc"}), Some(json!(2)));
        stub.accept_command("getblock", json!({"height": 200}), Some(json!(3)));

        let getblock_cmds = stub.get_commands_by_method("getblock");
        assert_eq!(getblock_cmds.len(), 2);
        assert_eq!(getblock_cmds[0].params, json!({"height": 100}));
        assert_eq!(getblock_cmds[1].params, json!({"height": 200}));

        let gettx_cmds = stub.get_commands_by_method("gettransaction");
        assert_eq!(gettx_cmds.len(), 1);
        assert_eq!(gettx_cmds[0].params, json!({"txid": "abc"}));
    }

    #[test]
    fn test_invalid_json() {
        let stub = CommandStore::new_without_logging();

        let result = stub.accept_json("invalid json", None);
        assert!(result.is_err());
        assert_eq!(stub.command_count(), 0);
    }

    #[test]
    fn test_logging_toggle() {
        let mut stub = CommandStore::new();
        assert!(stub.is_logging_enabled());

        stub.set_logging(false);
        assert!(!stub.is_logging_enabled());

        stub.set_logging(true);
        assert!(stub.is_logging_enabled());
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let stub = CommandStore::new_without_logging();
        let stub_clone = stub.clone();

        let handle = thread::spawn(move || {
            for i in 0..10 {
                stub_clone.accept_command(
                    format!("method_{}", i),
                    json!({"index": i}),
                    Some(json!(i)),
                );
            }
        });

        for i in 10..20 {
            stub.accept_command(format!("method_{}", i), json!({"index": i}), Some(json!(i)));
        }

        handle.join().unwrap();
        assert_eq!(stub.command_count(), 20);
    }
}
