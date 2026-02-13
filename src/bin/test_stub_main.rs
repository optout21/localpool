use localpool::test_stub::RpcStub;

use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse port from command line, default to 8332
    let port: u16 = if args.len() >= 2 {
        args[1].parse().unwrap_or_else(|_| {
            eprintln!(
                "Error: Invalid port number '{}', using default 8332",
                args[1]
            );
            8332
        })
    } else {
        8332
    };

    println!("RPC Stub Server");
    println!("===============");
    println!("Port: {}", port);
    println!("Logging: enabled");
    println!();
    println!("Press Ctrl+C to stop the server...");
    println!();

    // Create the RPC stub with logging enabled
    let mut stub = RpcStub::new(true, port);

    // Set up signal handler for graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                println!("\nReceived Ctrl+C, shutting down...");
                r.store(false, Ordering::SeqCst);
            }
            Err(err) => {
                eprintln!("Error setting up signal handler: {}", err);
            }
        }
    });

    // Keep the server running until Ctrl+C is pressed
    while running.load(Ordering::SeqCst) {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Stop the server
    stub.stop();
    println!("Server stopped.");
}
