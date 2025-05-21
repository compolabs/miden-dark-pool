use miden_client::keystore::FilesystemKeyStore;
use std::process::Command;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

use miden_client::transaction::TransactionRequestBuilder;
use miden_dark_pool::cli::open_order::get_serial_num;
use miden_dark_pool::utils::common::client_setup;
use miden_dark_pool::utils::common::create_partial_swap_note;
use miden_dark_pool::utils::common::delete_keystore_and_store;
use miden_dark_pool::utils::test_utils::TestUser;
use miden_dark_pool::utils::test_utils::setup_test_user;
use miden_dark_pool::utils::utility::create_faucet;
use miden_objects::asset::FungibleAsset;
use miden_objects::transaction::OutputNote;


#[tokio::test]
async fn test_open_order() {
    // Launch test matcher server in background
    tokio::spawn(async {
        let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
        let (_, _) = listener.accept().await.unwrap();
    });

    // Wait to ensure matcher is ready
    sleep(Duration::from_secs(5)).await;

    let mut client = client_setup().await.unwrap();

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
        &mut client,
        keystore,
        &format!("testuser"),
        faucet_a.clone(),
        faucet_b.clone(),
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
            "open-order",
            "--user-id",
            users[0].account_id.id().to_hex().as_str(),
            "--offered-asset",
            faucet_a.id().to_hex().as_str(),
            "--offered-amount",
            "50",
            "--requested-asset",
            faucet_b.id().to_hex().as_str(),
            "--price",
            "100",
        ])
        .output()
        .expect("Failed to execute user binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);

    assert!(output.status.success(), "User binary failed");
    assert!(stdout.contains("Note sent"));
    delete_keystore_and_store().await;
}

#[tokio::test]
async fn test_cancel_order() {
    let mut client = client_setup().await.unwrap();

    let keystore = FilesystemKeyStore::new("./keystore".into()).unwrap();

    let symbol = "ETH";
    let faucet_a = create_faucet(&mut client, keystore.clone(), symbol)
        .await
        .unwrap();

    let symbol = "BTC";
    let faucet_b = create_faucet(&mut client, keystore.clone(), symbol)
        .await
        .unwrap();

    // creates a user account and mints and consumes tokens(faucet_a: 100 and faucet_b: 20)
    let user = setup_test_user(
        &mut client,
        keystore,
        &format!("testuser"),
        faucet_a.clone(),
        faucet_b.clone(),
        100,
    )
    .await;

    let asset_a = FungibleAsset::new(faucet_a.id(), 50).unwrap();
    let asset_b = FungibleAsset::new(faucet_b.id(), 50).unwrap();

    let acc = client
        .get_account(user.account_id.id())
        .await
        .unwrap()
        .unwrap();
    let balance1 = acc.account().vault().get_balance(faucet_a.id()).unwrap();
    let balance2 = acc.account().vault().get_balance(faucet_b.id()).unwrap();
    assert!(balance1 == 100);
    assert!(balance2 == 20);

    // Set up the swap transaction
    let serial_num = get_serial_num(user.account_id.id());
    let fill_number = 0;

    // Create the partial swap note
    let swap_note = create_partial_swap_note(
        user.account_id.id(),
        user.account_id.id(),
        asset_a.into(),
        asset_b.into(),
        serial_num,
        fill_number,
    )
    .unwrap();

    let note_req = TransactionRequestBuilder::new()
        .with_own_output_notes(vec![OutputNote::Full(swap_note.clone())])
        .build()
        .unwrap();

    let tx_result = client
        .new_transaction(user.account_id.id(), note_req)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result).await;
    client.sync_state().await.unwrap();

    let acc = client
        .get_account(user.account_id.id())
        .await
        .unwrap()
        .unwrap();
    let balance1 = acc.account().vault().get_balance(faucet_a.id()).unwrap();
    let balance2 = acc.account().vault().get_balance(faucet_b.id()).unwrap();
    assert!(balance1 == 50);
    assert!(balance2 == 20);

    // Call the user binary
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--bin",
            "user",
            "--",
            "cancel-order",
            "--user-id",
            user.account_id.id().to_hex().as_str(),
            "--order-id",
            swap_note.id().to_hex().as_str(),
            "--tag",
            swap_note.metadata().tag().to_string().as_str(),
        ])
        .output()
        .expect("Failed to execute user binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("stdout:\n{}", stdout);
    println!("stderr:\n{}", stderr);

    assert!(output.status.success(), "User binary failed");
    assert!(stdout.contains("true"));

    let acc = client
        .get_account(user.account_id.id())
        .await
        .unwrap()
        .unwrap();
    let balance1 = acc.account().vault().get_balance(faucet_a.id()).unwrap();
    println!("Balance1: {}", balance1);
    let balance2 = acc.account().vault().get_balance(faucet_b.id()).unwrap();
    println!("Balance2: {}", balance2);

    assert!(balance1 == 100);
    assert!(balance2 == 20);

    delete_keystore_and_store().await;
}
