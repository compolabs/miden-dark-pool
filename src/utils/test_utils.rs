use super::utility::{create_account, mint_and_consume};
use miden_client::account::Account;
use miden_client::{
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
};
use std::sync::Arc;

pub struct TestUser {
    user_id: String,
    account_id: Account,
    keystore_path: String,
    store_path: String,
}

async fn setup_test_user(user_id: &str, faucet_a: Account, amount: u64) -> TestUser {
    let keystore_path = format!("./keystore_{}", user_id);
    let store_path = format!("./store_{}.sqlite3", user_id);

    let endpoint = Endpoint::new(
        "https".to_string(),
        "rpc.testnet.miden.io".to_string(),
        Some(443),
    );
    let rpc_api = Arc::new(TonicRpcClient::new(&endpoint, 10_000));

    let mut client = ClientBuilder::new()
        .with_rpc(rpc_api.clone())
        .with_filesystem_keystore(&keystore_path)
        .with_sqlite_store(&store_path)
        .in_debug_mode(true)
        .build()
        .await
        .unwrap();

    let keystore = FilesystemKeyStore::new(keystore_path.clone().into()).unwrap();
    let account = create_account(&mut client, keystore.clone()).await.unwrap();

    // Mint token A to the user
    mint_and_consume(&mut client, faucet_a, account.clone(), amount)
        .await
        .unwrap();

    TestUser {
        user_id: user_id.to_string(),
        account_id: account,
        keystore_path,
        store_path,
    }
}
