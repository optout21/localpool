use crate::rpc_command::RpcCommand;
use crate::test_stub::command_store::CommandStore;

use bytes::Bytes;
use serde_json::Value;
use tokio::sync::oneshot;
use warp::Filter;

/// A test stub for handling JSON-RPC commands
///
/// RpcStub can accept JSON RPC commands, store them, log them,
/// and return them upon request for testing purposes.
#[derive(Debug)]
pub struct RpcStub {
    /// store the received commands
    command_store: CommandStore,
    /// shutdown signal sender
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// port the server is listening on
    port: u16,
}

impl RpcStub {
    /// Creates a new RpcStub with logging enabled by default and starts HTTP server
    pub fn new(logging: bool, port: u16) -> Self {
        let command_store = if logging {
            CommandStore::new()
        } else {
            CommandStore::new_without_logging()
        };

        let mut stub = Self {
            command_store,
            shutdown_tx: None,
            port,
        };

        stub.start_server();
        stub
    }

    /// Starts the HTTP server on the configured port
    fn start_server(&mut self) {
        let command_store = self.command_store.clone();
        let port = self.port;

        let (tx, rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(tx);

        // Create the RPC endpoint
        let rpc_route = warp::post()
            .and(warp::path::end())
            .and(
                warp::body::json()
                    .or(warp::body::bytes().and_then(|bytes: Bytes| async move {
                        serde_json::from_slice(&bytes).map_err(|_| warp::reject::reject())
                    }))
                    .unify(),
            )
            .map(move |body: serde_json::Value| {
                // Extract method, params, and id from the JSON-RPC request
                let method = body
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let params = body
                    .get("params")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);

                let id = body.get("id").cloned();

                // Store the command
                command_store.accept_command(method.clone(), params.clone(), id.clone());

                let response = Self::handle_command(method.clone(), params.clone(), id.clone());

                warp::reply::json(&response)
            });

        // Spawn the server in a background thread with its own runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let (_, server) = warp::serve(rpc_route).bind_with_graceful_shutdown(
                    ([127, 0, 0, 1], port),
                    async {
                        rx.await.ok();
                    },
                );

                server.await;
            });
        });

        // Give the server a moment to start
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    fn handle_command(method: String, _params: Value, id: Option<Value>) -> Value {
        match method.as_str() {
            "getblockchaininfo" => {
                // Return a syntactically correct response with dummy values, a canned response
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "chain": "main",
                        "blocks": 935709,
                        "headers": 935709,
                        "bestblockhash": "0000000000000000000086fe82dcaf9b7731e847ad94963174eeb0334a8854c5",
                        "bits": "17023c7e",
                        "target": "000000000000000000023c7e0000000000000000000000000000000000000000",
                        "difficulty": 125864590119494.3,
                        "time": 1770622337,
                        "mediantime": 1770620433,
                        "verificationprogress": 0.9999999505236595,
                        "initialblockdownload": false,
                        "chainwork": "00000000000000000000000000000000000000010db1f398d45a42a05b50e0c4",
                        "size_on_disk": 818671636141i64,
                        "pruned": false,
                        "warnings": [],
                    },
                    "id": id,
                })
            }
            _ => {
                // Return a simple JSON-RPC response
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": "ok",
                    "id": id,
                })
            }
        }
    }

    /// Stops the HTTP server
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Give the server a moment to shutdown
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    /// Returns the port the server is listening on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Returns all stored commands
    pub fn get_commands(&self) -> Vec<RpcCommand> {
        self.command_store.get_commands()
    }

    /// Returns the last command received, if any
    pub fn get_last_command(&self) -> Option<RpcCommand> {
        self.command_store.get_last_command()
    }

    /// Returns all commands with a specific method name
    pub fn get_commands_by_method(&self, method: &str) -> Vec<RpcCommand> {
        self.command_store.get_commands_by_method(method)
    }

    /// Returns the number of commands received
    pub fn command_count(&self) -> usize {
        self.command_store.command_count()
    }

    /// Clears all stored commands
    pub fn clear(&self) {
        self.command_store.clear();
    }
}

impl Default for RpcStub {
    fn default() -> Self {
        Self::new(false, 8332)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn create_and_start_stub(port: u16) -> RpcStub {
        RpcStub::new(false, port)
    }

    fn stop_stub(stub: &mut RpcStub) {
        stub.stop();
    }

    #[tokio::test]
    async fn test_http_server_accepts_rpc_command() {
        let mut stub = create_and_start_stub(18332);

        // Send an HTTP POST request with JSON-RPC command
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .header("Content-Type", "text/plain")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "getblockcount",
                "params": {},
                "id": 1
            }))
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());

        // Verify the command was stored
        assert_eq!(stub.command_count(), 1);
        let last_cmd = stub.get_last_command().unwrap();
        assert_eq!(last_cmd.method, "getblockcount");
        assert_eq!(last_cmd.id, Some(json!(1)));

        stop_stub(&mut stub);
    }

    #[tokio::test]
    async fn test_http_server_multiple_commands() {
        let mut stub = create_and_start_stub(18333);

        let client = reqwest::Client::new();

        // Send first command
        client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "getblock",
                "params": {"hash": "abc123"},
                "id": 1
            }))
            .send()
            .await
            .unwrap();

        // Send second command
        client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "gettransaction",
                "params": {"txid": "def456"},
                "id": 2
            }))
            .send()
            .await
            .unwrap();

        // Verify both commands were stored
        assert_eq!(stub.command_count(), 2);

        let commands = stub.get_commands();
        assert_eq!(commands[0].method, "getblock");
        assert_eq!(commands[0].params, json!({"hash": "abc123"}));
        assert_eq!(commands[1].method, "gettransaction");
        assert_eq!(commands[1].params, json!({"txid": "def456"}));

        stop_stub(&mut stub);
    }

    #[tokio::test]
    async fn test_http_server_filter_by_method() {
        let mut stub = create_and_start_stub(18334);

        let client = reqwest::Client::new();

        // Send multiple commands with different methods
        client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "getblock",
                "params": {"height": 100},
                "id": 1
            }))
            .send()
            .await
            .unwrap();

        client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "gettransaction",
                "params": {"txid": "abc"},
                "id": 2
            }))
            .send()
            .await
            .unwrap();

        client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "getblock",
                "params": {"height": 200},
                "id": 3
            }))
            .send()
            .await
            .unwrap();

        // Verify filtering works
        let getblock_cmds = stub.get_commands_by_method("getblock");
        assert_eq!(getblock_cmds.len(), 2);
        assert_eq!(getblock_cmds[0].params, json!({"height": 100}));
        assert_eq!(getblock_cmds[1].params, json!({"height": 200}));

        let gettx_cmds = stub.get_commands_by_method("gettransaction");
        assert_eq!(gettx_cmds.len(), 1);
        assert_eq!(gettx_cmds[0].params, json!({"txid": "abc"}));

        stop_stub(&mut stub);
    }

    #[tokio::test]
    async fn test_http_server_clear_commands() {
        let mut stub = create_and_start_stub(18335);

        let client = reqwest::Client::new();

        // Send a command
        client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "test",
                "params": {},
                "id": 1
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(stub.command_count(), 1);

        // Clear commands
        stub.clear();
        assert_eq!(stub.command_count(), 0);
        assert!(stub.get_last_command().is_none());

        stop_stub(&mut stub);
    }

    #[tokio::test]
    async fn test_http_server_response_format() {
        let mut stub = create_and_start_stub(18336);

        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "getinfo",
                "params": {},
                "id": 42
            }))
            .send()
            .await
            .unwrap();

        // Verify response format
        let body: serde_json::Value = response.json::<Value>().await.unwrap();

        assert_eq!(body.get("jsonrpc").unwrap(), "2.0");
        assert_eq!(body.get("result").unwrap(), "ok");
        assert_eq!(body.get("id").unwrap(), &json!(42));

        stop_stub(&mut stub);
    }

    #[tokio::test]
    async fn test_getblockchaininfo_response() {
        let mut stub = create_and_start_stub(18337);

        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://127.0.0.1:{}", stub.port()))
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "getblockchaininfo",
                "params": {},
                "id": 1
            }))
            .send()
            .await
            .unwrap();

        assert!(response.status().is_success());

        // Verify the response contains blockchain info
        let body: serde_json::Value = response.json::<Value>().await.unwrap();

        assert_eq!(body.get("jsonrpc").unwrap(), "2.0");
        assert_eq!(body.get("id").unwrap(), &json!(1));

        // Check the result object
        let result = body.get("result").unwrap();
        assert!(result.is_object());

        // Verify the blocks field exists and has the expected value
        assert_eq!(result.get("blocks").unwrap(), 935709);
        assert_eq!(result.get("chain").unwrap(), "main");
        assert_eq!(result.get("headers").unwrap(), 935709);
        assert!(result.get("bestblockhash").is_some());
        assert!(result.get("difficulty").is_some());

        // Verify the command was stored
        assert_eq!(stub.command_count(), 1);
        let last_cmd = stub.get_last_command().unwrap();
        assert_eq!(last_cmd.method, "getblockchaininfo");

        stop_stub(&mut stub);
    }
}
