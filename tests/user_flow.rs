use miden_client::builder::ClientBuilder;
use miden_client::keystore::FilesystemKeyStore;
use miden_client::rpc::Endpoint;
use miden_client::rpc::TonicRpcClient;
use std::process::Command;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

use miden_dark_pool::utils::test_utils::TestUser;
use miden_dark_pool::utils::test_utils::setup_test_user;
use miden_dark_pool::utils::utility::create_faucet;

use std::sync::Arc;

#[tokio::test]
async fn test_user_flow() {
    // Launch test matcher server in background
    tokio::spawn(async {
        let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
        let (_, _) = listener.accept().await.unwrap();
    });

    // Wait to ensure matcher is ready
    sleep(Duration::from_secs(5)).await;

    let endpoint = Endpoint::new(
        "https".to_string(),
        "rpc.testnet.miden.io".to_string(),
        Some(443),
    );
    let timeout_ms = 10_000;
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, timeout_ms));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api)
        .with_filesystem_keystore("./keystore")
        .in_debug_mode(true)
        .build()
        .await
        .unwrap();

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    let symbol = "ETH";
    let faucet_a = create_faucet(&mut client, keystore.clone(), symbol)
        .await
        .unwrap();

    let symbol = "BTC";
    let faucet_b = create_faucet(&mut client, keystore.clone(), symbol)
        .await
        .unwrap();

    let mut users: Vec<TestUser> = Vec::new();

    let user = setup_test_user(
        client,
        keystore,
        &format!("testuser"),
        faucet_a.clone(),
        100,
    )
    .await;
    users.push(user);

    // Call the user binary
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--bin",
            "user",
            "--",
            "--user",
            users[0].account_id.id().to_hex().as_str(),
            "--token-a",
            faucet_a.id().to_hex().as_str(),
            "--amount-a",
            "50",
            "--token-b",
            faucet_b.id().to_hex().as_str(),
            "--matcher-addr",
            "127.0.0.1:8080",
        ])
        .output()
        .expect("Failed to execute user binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);

    assert!(output.status.success(), "User binary failed");
    assert!(stdout.contains("Note sent!"));
}
