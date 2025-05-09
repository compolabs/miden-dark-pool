use super::utility::{create_account, mint_and_consume};
use miden_client::Client;
use miden_client::account::Account;
use miden_client::keystore::FilesystemKeyStore;
use rand::rngs::StdRng;

//TODO: not a dead code
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TestUser {
    pub user_id: String,
    pub account_id: Account,
}

//TODO: not a dead code
#[allow(dead_code)]
pub async fn setup_test_user(
    mut client: &mut Client,
    keystore: FilesystemKeyStore<StdRng>,
    user_id: &str,
    faucet_a: Account,
    faucet_b: Account,
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

    mint_and_consume(&mut client, faucet_b, account.clone(), 20)
        .await
        .unwrap();

    let _ = client.sync_state().await.unwrap();

    TestUser {
        user_id: user_id.to_string(),
        account_id: account,
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::utility::create_faucet;
    use miden_client::builder::ClientBuilder;
    use miden_client::rpc::Endpoint;
    use miden_client::rpc::TonicRpcClient;
    use std::sync::Arc;

    use super::*;
    #[ignore = "Taking significant time"]
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

        let faucet_b = create_faucet(&mut client, keystore.clone(), symbol)
            .await
            .unwrap();

        let mut users: Vec<TestUser> = Vec::new();

        let user = setup_test_user(
            &mut client,
            keystore.clone(),
            &format!("testuser"),
            faucet_a.clone(),
            faucet_b.clone(),
            100,
        )
        .await;
        users.push(user);
    }
}
