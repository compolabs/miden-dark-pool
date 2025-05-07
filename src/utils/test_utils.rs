use super::utility::{create_account, mint_and_consume};
use miden_client::account::Account;
use miden_client::{Client, keystore};
use miden_client::{
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
};
use rand::rngs::StdRng;
use std::sync::Arc;

pub struct TestUser {
    pub user_id: String,
    pub account_id: Account,
}

pub async fn setup_test_user(
    mut client: Client,
    keystore: FilesystemKeyStore<StdRng>,
    user_id: &str,
    faucet_a: Account,
    amount: u64,
) -> TestUser {
    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let account = create_account(&mut client, keystore.clone()).await.unwrap();

    client.sync_state().await.unwrap();
    // Mint token A to the user
    mint_and_consume(&mut client, faucet_a, account.clone(), amount)
        .await
        .unwrap();

    TestUser {
        user_id: user_id.to_string(),
        account_id: account,
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::utility::create_faucet;

    use super::*;

    #[tokio::test]
    async fn test_setup() {
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

        let mut users: Vec<TestUser> = Vec::new();

        let user = setup_test_user(
            client,
            keystore.clone(),
            &format!("testuser"),
            faucet_a.clone(),
            100,
        )
        .await;
        users.push(user);
    }
}
