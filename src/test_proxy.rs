use std::time::Duration;

use crate::rpc_proxy::{ProxyConfig, RpcProxy};
use crate::rpc_stub::RpcStub;
use serde_json::json;

fn create_and_start_stub(stub_port: u16) -> (String, RpcStub) {
    let stub = RpcStub::new(false, stub_port);
    let stub_url = format!("http://127.0.0.1:{}", stub_port);
    (stub_url, stub)
}

async fn create_and_start_stub_and_proxy(stub_port: u16, proxy_port: u16) -> (RpcStub, RpcProxy) {
    let (stub_url, stub) = create_and_start_stub(stub_port);
    let proxy_config = ProxyConfig::new(proxy_port, &stub_url);
    let proxy = RpcProxy::new_with_config(proxy_config).await;
    (stub, proxy)
}

async fn create_and_start_stub_and_proxy_with_config(
    stub_port: u16,
    proxy_port: u16,
    mut proxy_config: ProxyConfig,
) -> (RpcStub, RpcProxy) {
    let (stub_url, stub) = create_and_start_stub(stub_port);
    proxy_config.port = proxy_port;
    proxy_config.upstream_url = stub_url;
    let proxy = RpcProxy::new_with_config(proxy_config).await;
    (stub, proxy)
}

fn stop_proxy_and_stub(stub: &mut RpcStub, proxy: &mut RpcProxy) {
    stub.stop();
    proxy.stop();
}

#[tokio::test]
async fn test_proxy_accepts_rpc_command() {
    let (mut stub, mut proxy) = create_and_start_stub_and_proxy(8342, 18342).await;

    // Send an HTTP POST request with JSON-RPC command
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}", proxy.port()))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "getblockcount",
            "params": {},
            "id": "1"
        }))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    // Verify the command was accepted by the stub
    assert_eq!(stub.command_count(), 1);
    let last_cmd = stub.get_last_command().unwrap();
    assert_eq!(last_cmd.method, "getblockcount");
    assert_eq!(last_cmd.id, Some(json!("1")));

    stop_proxy_and_stub(&mut stub, &mut proxy);
}

#[tokio::test]
async fn test_proxy_as_plaintext() {
    let (mut stub, mut proxy) = create_and_start_stub_and_proxy(8343, 18343).await;

    // Send an HTTP POST request with JSON-RPC command
    let client = reqwest::Client::new();

    let command_text = json!({
        "jsonrpc": "2.0",
        "method": "getblockcount",
        "params": {},
        "id": "1"
    })
    .to_string();

    let response = client
        .post(format!("http://127.0.0.1:{}", proxy.port()))
        .header("Content-Type", "text/plain")
        .body(command_text)
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());

    // Verify the command was accepted by the stub
    assert_eq!(stub.command_count(), 1);
    let last_cmd = stub.get_last_command().unwrap();
    assert_eq!(last_cmd.method, "getblockcount");
    assert_eq!(last_cmd.id, Some(json!("1")));

    stop_proxy_and_stub(&mut stub, &mut proxy);
}

#[tokio::test]
async fn test_proxy_multiple_commands() {
    let (mut stub, mut proxy) = create_and_start_stub_and_proxy(8344, 18344).await;

    let client = reqwest::Client::new();

    // Send first command
    client
        .post(format!("http://127.0.0.1:{}", proxy.port()))
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
        .post(format!("http://127.0.0.1:{}", proxy.port()))
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

    stop_proxy_and_stub(&mut stub, &mut proxy);
}

#[tokio::test]
async fn test_proxy_sendrawtransaction_delayed() {
    let mut config = ProxyConfig::new(1, "1");
    config.delayed_broadcast = true;
    config.delayed_broadcast_delay_secs = 3600;
    let (mut stub, mut proxy) =
        create_and_start_stub_and_proxy_with_config(8345, 18345, config).await;

    let client = reqwest::Client::new();

    client
        .post(format!("http://127.0.0.1:{}", proxy.port()))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "sendrawtransaction",
            "params": ["0123456789abcdef"],
            "id": 2
        }))
        .send()
        .await
        .unwrap();

    // Not forwarded immediately
    assert_eq!(stub.command_count(), 0);
    assert_eq!(proxy.get_state().in_cmd_cnt, 1);
    assert_eq!(proxy.get_state().local_txs.len(), 1);

    // jump time by a little
    proxy.test_bump_clock(60);
    // Still not forwarded
    assert_eq!(stub.command_count(), 0);
    assert_eq!(proxy.get_state().local_txs.len(), 1);

    // jump time past the target time
    proxy.test_bump_clock(3700);
    // Should be broadcast, check in loop
    for i in 1..100 {
        std::thread::sleep(Duration::from_millis(i));
        proxy.test_bump_clock(1);
        print!(".");
        if stub.command_count() >= 1 {
            break;
        }
    }
    assert_eq!(stub.command_count(), 1);
    assert_eq!(proxy.get_state().local_txs.len(), 0);

    stop_proxy_and_stub(&mut stub, &mut proxy);
}

#[tokio::test]
async fn test_proxy_sendrawtransaction_nondelayed() {
    let (mut stub, mut proxy) = create_and_start_stub_and_proxy(8346, 18346).await;

    let client = reqwest::Client::new();

    client
        .post(format!("http://127.0.0.1:{}", proxy.port()))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "sendrawtransaction",
            "params": ["0123456789abcdef"],
            "id": 2
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(stub.command_count(), 1);

    let commands = stub.get_commands();
    assert_eq!(commands[0].method, "sendrawtransaction");

    assert_eq!(proxy.get_state().in_cmd_cnt, 1);
    assert_eq!(proxy.get_state().local_txs.len(), 0);

    stop_proxy_and_stub(&mut stub, &mut proxy);
}
