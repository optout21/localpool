use crate::local_tx::LocalTx;
use crate::local_txs::LocalTxs;
use crate::rpc_command::RpcCommand;

use bytes::Bytes;
use serde_json::{Value, json};
use tokio::sync::oneshot;
use warp::Filter;

use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// The upstream server URL, we forward here
    pub upstream_url: String,
    /// port the server is listening on
    pub port: u16,
    pub delayed_broadcast: bool,
    pub delayed_broadcast_delay_secs: u32,
}

impl ProxyConfig {
    pub fn new(local_port: u16, upstream_url: &str) -> Self {
        Self {
            upstream_url: upstream_url.into(),
            port: local_port,
            delayed_broadcast: false,
            delayed_broadcast_delay_secs: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyState {
    pub in_cmd_cnt: u32,
    pub local_txs: LocalTxs,
    /// During tests, advance/bump the clock forward (secs)
    #[cfg(test)]
    pub test_clock_advance: u32,
}

impl ProxyState {
    pub fn new() -> Self {
        Self {
            in_cmd_cnt: 0,
            local_txs: LocalTxs::new(),
            #[cfg(test)]
            test_clock_advance: 0,
        }
    }

    pub fn get_earliest_tx(&self) -> Option<(u32, LocalTx)> {
        match self.local_txs.get_earliest() {
            None => None,
            Some(tx) => Some((tx.target_time(), tx)),
        }
    }

    fn now(&self) -> u32 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should go forward")
            .as_secs() as u32;
        #[cfg(test)]
        return now + self.test_clock_advance;
        #[cfg(not(test))]
        now
    }

    /// Advance (bump) the clock, for testing. Seconds to jump forward.
    #[cfg(test)]
    pub fn test_bump_clock(&mut self, jump_secs: u32) {
        self.test_clock_advance += jump_secs;
    }
}

/// A proxy forwarding JSON-RPC commands
///
/// RpcProxy can accept JSON RPC commands, store them, log them,
/// and return them upon request for testing purposes.
#[derive(Debug)]
pub struct RpcProxy {
    config: ProxyConfig,
    /// shutdown signal sender for HTTP server
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// shutdown signal for broadcast thread
    broadcast_shutdown: Arc<(Mutex<bool>, Condvar)>,
    state: Arc<RwLock<ProxyState>>,
}

impl RpcProxy {
    pub async fn new_with_config(config: ProxyConfig) -> Self {
        // // Test upstream connection BEFORE starting the server
        // if let Err(e) = Self::test_upstream_connection(upstream_url).await {
        //     panic!("Upstream server test failed, {}", e);
        // }

        let broadcast_shutdown = Arc::new((Mutex::new(false), Condvar::new()));

        let mut proxy = Self {
            config: config.clone(),
            shutdown_tx: None,
            broadcast_shutdown: broadcast_shutdown.clone(),
            state: Arc::new(RwLock::new(ProxyState::new())),
        };

        proxy.start_server();

        // Start the broadcast thread if delayed_broadcast is enabled
        if config.delayed_broadcast {
            proxy.start_broadcast_thread();
        }

        proxy
    }

    /// Creates a new RpcProxy with logging enabled by default and starts HTTP server
    pub async fn new(local_port: u16, upstream_url: &str) -> Self {
        let config = ProxyConfig::new(local_port, upstream_url);
        Self::new_with_config(config).await
    }

    /*
    /// Tests the upstream server connection by calling getblockchaininfo
    async fn test_upstream_connection(upstream_url: &str) -> Result<(), String> {
        println!("Testing upstream server connection at {}...", upstream_url);

        let client = reqwest::Client::new();
        let response = client
            .post(upstream_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "getblockchaininfo",
                "params": [],
                "id": "test",
            }))
            .send()
            .await;

        let result = match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<Value>().await {
                        Ok(json_response) => {
                            if let Some(result) = json_response.get("result") {
                                let chain = result
                                    .get("chain")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                let blocks =
                                    result.get("blocks").and_then(|v| v.as_u64()).unwrap_or(0);

                                println!("✓ Upstream server connected successfully!");
                                println!("  Chain: {}  Blocks: {}", chain, blocks);
                                Ok(())
                            } else if let Some(error) = json_response.get("error") {
                                Err(format!("RPC error: {}", error))
                            } else {
                                Err("Invalid response format".to_string())
                            }
                        }
                        Err(e) => Err(format!("Failed to parse JSON response: {}", e)),
                    }
                } else {
                    Err(format!("HTTP error: {}", resp.status()))
                }
            }
            Err(e) => Err(format!("Connection failed: {}", e)),
        };

        if let Err(e) = &result {
            eprintln!("✗ Failed to connect to upstream server: {}", e);
        }

        result
    }
    */

    /// Starts the HTTP server on the configured port
    fn start_server(&mut self) {
        let port = self.config.port;
        let config = self.config.clone();
        let state = self.state.clone();
        let broadcast_shutdown = self.broadcast_shutdown.clone();

        let (tx, rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(tx);

        // Create the RPC endpoint - accept both application/json and text/plain
        let rpc_route = warp::post()
            .and(warp::path::end())
            .and(warp::header::headers_cloned())
            .and(
                warp::body::json()
                    .or(warp::body::bytes().and_then(|bytes: Bytes| async move {
                        serde_json::from_slice(&bytes).map_err(|_| warp::reject::reject())
                    }))
                    .unify(),
            )
            .and_then(
                move |headers: warp::http::HeaderMap, body: serde_json::Value| {
                    let config = config.clone();
                    let state = state.clone();
                    let broadcast_shutdown = broadcast_shutdown.clone();
                    async move {
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
                        // Extract all headers from the incoming request
                        let heads: Vec<(String, String)> = headers
                            .iter()
                            .filter(|(_k, v)| !v.is_empty())
                            .map(|(k, v)| {
                                (
                                    k.as_str().to_string(),
                                    v.to_str().unwrap_or("").to_string().clone(),
                                )
                            })
                            .collect();

                        let command = RpcCommand {
                            method,
                            params,
                            id,
                            headers: Some(heads),
                        };

                        // Forward to upstream with headers
                        let response =
                            Self::handle_command(&config, &state, &command, &broadcast_shutdown)
                                .await;

                        Ok::<_, warp::Rejection>(warp::reply::json(&response))
                    }
                },
            );

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
        // std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // fn now(&self) -> u32 {
    //     SystemTime::now()
    //                         .duration_since(UNIX_EPOCH)
    //                         .expect("time should go forward")
    //                         .as_secs() as u32
    // }

    /// Starts the background thread that broadcasts transactions at their target time
    fn start_broadcast_thread(&self) {
        let state = self.state.clone();
        let config = self.config.clone();
        let shutdown = self.broadcast_shutdown.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();

            loop {
                // Check if we should shutdown
                let (lock, _cvar) = &*shutdown;
                let should_stop = *lock.lock().unwrap();
                if should_stop {
                    break;
                }

                // Get the earliest transaction
                let earliest_opt = state.read().unwrap().get_earliest_tx();

                match earliest_opt {
                    None => {
                        // No transactions, wait for signal or timeout
                        let (lock, cvar) = &*shutdown;
                        let mut should_stop = lock.lock().unwrap();

                        // Wait for up to 1 second or until signaled
                        let result = cvar
                            .wait_timeout(should_stop, Duration::from_secs(1))
                            .unwrap();
                        should_stop = result.0;

                        if *should_stop {
                            break;
                        }
                    }
                    Some((target_time, _tx)) => {
                        // Calculate how long to wait
                        let now = state.read().unwrap().now();
                        if target_time <= now {
                            // Time to broadcast! Remove and send the transaction
                            let tx_to_broadcast =
                                state.write().unwrap().local_txs.remove_earliest();

                            if let Some(tx) = tx_to_broadcast {
                                // Broadcast the transaction to upstream
                                rt.block_on(async {
                                    let command = tx.get_command();
                                    let _response =
                                        Self::forward_to_upstream(&config, &command).await;
                                    // TODO: Handle response, log errors, etc.
                                    println!(
                                        "Broadcasted transaction at target_time: {}",
                                        tx.target_time()
                                    );
                                });
                            }
                        } else {
                            // Wait until target time or until signaled
                            let wait_duration = Duration::from_secs((target_time - now) as u64);

                            let (lock, cvar) = &*shutdown;
                            let mut should_stop = lock.lock().unwrap();

                            // Wait for the calculated duration or until signaled
                            let result = cvar.wait_timeout(should_stop, wait_duration).unwrap();
                            should_stop = result.0;

                            if *should_stop {
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    /// Stops the HTTP server and broadcast thread
    pub fn stop(&mut self) {
        // Signal broadcast thread to stop
        let (lock, cvar) = &*self.broadcast_shutdown;
        {
            let mut should_stop = lock.lock().unwrap();
            *should_stop = true;
        }
        cvar.notify_all();

        // Stop HTTP server
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Give the servers a moment to shutdown
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    /// Returns the port the server is listening on
    pub fn port(&self) -> u16 {
        self.config.port
    }

    pub fn get_state(&self) -> ProxyState {
        self.state.read().unwrap().clone()
    }

    /// Advance (bump) the clock, for testing. Seconds to jump forward.
    #[cfg(test)]
    pub fn test_bump_clock(&mut self, jump_secs: u32) {
        self.state.write().unwrap().test_bump_clock(jump_secs);

        // Wake up broadcast thread
        let (_lock, cvar) = &*self.broadcast_shutdown;
        {
            // let mut should_stop = lock.lock().unwrap();
            // *should_stop = true;
        }
        cvar.notify_all();
    }

    async fn forward_to_upstream(config: &ProxyConfig, command: &RpcCommand) -> Value {
        // Forward to upstream with headers
        let client = reqwest::Client::new();
        let mut request = client.post(&config.upstream_url).json(&json!({
            "jsonrpc": "2.0",
            "method": command.method,
            "params": command.params,
            "id": json!(command.id),
        }));

        // Copy headers from the incoming request
        if let Some(heads) = &command.headers {
            for (key, value) in heads {
                let key = key.to_ascii_lowercase();
                if key != "host" && key != "content-length" {
                    // println!("Header: {} {}", key.as_str(), value_str);
                    request = request.header(key.as_str(), value);
                }
            }
            // println!("Headers set");
        }

        let response = request.send().await.unwrap();
        // println!("Response status: {:?}", response.status());
        if !response.status().is_success() {
            serde_json::json!({
                "jsonrpc": "2.0",
                "result": response.status().to_string(),
                "id": json!(command.id),
            })
        } else {
            // TODO handle error
            let result = response.json::<serde_json::Value>().await.unwrap();
            result
        }
    }

    fn get_signed_hex_from_params(command: &RpcCommand) -> Result<String, String> {
        if let serde_json::Value::Array(arr) = &command.params {
            if arr.len() >= 1 {
                if let serde_json::Value::String(s) = &arr[0] {
                    Ok(s.clone())
                } else {
                    Err("Couldn't find string parameters for signedhex".into())
                }
            } else {
                Err("Parameter array to short, for signedhex".into())
            }
        } else {
            Err("Couldn't find parameters array for signedhex".into())
        }
    }

    async fn handle_command(
        config: &ProxyConfig,
        state: &Arc<RwLock<ProxyState>>,
        command: &RpcCommand,
        broadcast_shutdown: &Arc<(Mutex<bool>, Condvar)>,
    ) -> Value {
        state.write().unwrap().in_cmd_cnt += 1;
        let res = match command.method.as_str() {
            "sendrawtransaction" => {
                let txhex = match Self::get_signed_hex_from_params(command) {
                    Err(_e) => return Self::forward_to_upstream(config, command).await,
                    Ok(txhex) => txhex,
                };
                if config.delayed_broadcast && config.delayed_broadcast_delay_secs > 0 {
                    let now = state.read().unwrap().now();
                    let target_time = now + config.delayed_broadcast_delay_secs;
                    state
                        .write()
                        .unwrap()
                        .local_txs
                        .add(txhex, target_time, command.clone());

                    // Wake up the broadcast thread to check the new transaction
                    let (_lock, cvar) = &**broadcast_shutdown;
                    cvar.notify_one();

                    json!({
                        "jsonrpc": "2.0",
                        "result": "Accepted, enqueued", // TODO
                        "id": json!(command.id),
                    })
                } else {
                    Self::forward_to_upstream(config, command).await
                }
            }
            _ => Self::forward_to_upstream(config, command).await,
        };
        res
    }
}
