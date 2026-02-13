use localpool::rpc_proxy::{ProxyConfig, RpcProxy};

use std::env;
use std::io::{self, BufRead};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <port> <upstream_url> <delay_secs>", args[0]);
        eprintln!("Example: {} 8432 http://127.0.0.1:8332", args[0]);
        eprintln!("  <port>          Port to listen on");
        eprintln!("  <upstream_url>  RPC address of the bitcoin node");
        eprintln!(
            "  <delay_secs>    The delay setting in seconds, in transparent mode. 0 for no delay"
        );
        std::process::exit(1);
    }

    let port: u16 = args[1].parse().unwrap_or_else(|_| {
        eprintln!("Error: Invalid port number '{}'", args[1]);
        std::process::exit(1);
    });

    let upstream_url = args[2].clone();

    let delay_secs = if args.len() >= 4 {
        args[3].parse().unwrap_or_else(|_| {
            eprintln!("Error: Invalid delay number '{}'", args[3]);
            std::process::exit(1);
        })
    } else {
        0
    };

    println!(
        "LocalPool  port: {}  upstream: {}  delay: {}",
        port, upstream_url, delay_secs
    );
    println!("  Upstream bitcoin node URL:   {}", upstream_url);
    println!("  Local port:                  {}", port);

    let mut proxy =
        RpcProxy::new_with_config(ProxyConfig::new(port, &upstream_url, delay_secs)).await;
    println!("Listening started ...");

    println!("Press Enter to stop the server...");
    // Wait for Enter key press
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let _ = lines.next();

    proxy.stop();
    println!("Listening stopped.");
}
