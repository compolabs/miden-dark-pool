use miden_client::Client;
use miden_client::account::Account;
use miden_client::keystore::FilesystemKeyStore;
use rand::rngs::StdRng;

use miden_client::ClientError;
use miden_client::account::{
    AccountBuilder, AccountStorageMode, AccountType, component::BasicFungibleFaucet,
    component::BasicWallet, component::RpoFalcon512,
};

use miden_objects::note::NoteType;
use miden_objects::{Felt, asset::FungibleAsset};

use miden_client::{
    asset::TokenSymbol, auth::AuthSecretKey, crypto::SecretKey,
    transaction::TransactionRequestBuilder,
};

use rand::RngCore;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TestUser {
    pub user_id: String,
    pub account_id: Account,
}

pub async fn create_account(
    client: &mut Client,
    keystore: FilesystemKeyStore<rand::prelude::StdRng>,
) -> Result<Account, ClientError> {
    let mut init_seed = [0_u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    let key_pair = SecretKey::with_rng(client.rng());

    // Anchor block
    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Public)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicWallet);

    let (alice_account, seed) = builder.build().unwrap();

    // Add the account to the client
    client
        .add_account(&alice_account, Some(seed), false)
        .await?;

    // Add the key pair to the keystore
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();
    Ok(alice_account)
}

// TODO: Not a dead code
#[allow(dead_code)]
pub async fn create_faucet(
    client: &mut Client,
    keystore: FilesystemKeyStore<rand::prelude::StdRng>,
    symbol: &str,
) -> Result<Account, ClientError> {
    // Faucet seed
    let mut init_seed = [0u8; 32];
    client.rng().fill_bytes(&mut init_seed);

    // Faucet parameters
    let symbol = TokenSymbol::new(symbol).unwrap();
    let decimals = 8;
    let max_supply = Felt::new(1_000_000);

    // Generate key pair
    let key_pair = SecretKey::with_rng(client.rng());

    let anchor_block = client.get_latest_epoch_block().await.unwrap();

    // Build the account
    let builder = AccountBuilder::new(init_seed)
        .anchor((&anchor_block).try_into().unwrap())
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_component(RpoFalcon512::new(key_pair.public_key()))
        .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).unwrap());

    let (faucet_account, seed) = builder.build().unwrap();

    // Add the faucet to the client
    client
        .add_account(&faucet_account, Some(seed), false)
        .await?;

    // Add the key pair to the keystore
    keystore
        .add_key(&AuthSecretKey::RpoFalcon512(key_pair))
        .unwrap();

    println!("Faucet account ID: {:?}", faucet_account.id().to_hex());

    // Resync to show newly deployed faucet
    client.sync_state().await?;
    Ok(faucet_account)
}

pub async fn mint_and_consume(
    client: &mut Client,
    faucet_account: Account,
    token_account: Account,
    amount: u64,
) -> Result<(), ClientError> {
    let fungible_asset = FungibleAsset::new(faucet_account.id(), amount).unwrap();

    let transaction_request = TransactionRequestBuilder::mint_fungible_asset(
        fungible_asset,
        token_account.id(),
        NoteType::Public,
        client.rng(),
    )
    .unwrap()
    .build()
    .unwrap();

    let tx_execution_result = client
        .new_transaction(faucet_account.id(), transaction_request)
        .await?;
    client.submit_transaction(tx_execution_result).await?;

    loop {
        // Resync to get the latest data
        client.sync_state().await?;

        let consumable_notes = client
            .get_consumable_notes(Some(token_account.id()))
            .await?;
        let list_of_note_ids: Vec<_> = consumable_notes.iter().map(|(note, _)| note.id()).collect();

        if list_of_note_ids.len() == 1 {
            let transaction_request = TransactionRequestBuilder::consume_notes(list_of_note_ids)
                .build()
                .unwrap();
            let tx_execution_result = client
                .new_transaction(token_account.id(), transaction_request)
                .await?;

            client.submit_transaction(tx_execution_result).await?;
            break;
        } else {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }

    client.sync_state().await?;
    Ok(())
}

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
    use miden_client::builder::ClientBuilder;
    use miden_client::rpc::Endpoint;
    use miden_client::rpc::TonicRpcClient;
    use std::sync::Arc;

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

pub async fn delete_keystore_and_store() {
    // Remove the SQLite store file

    let keystore_dir: &str = &format!("./keystore");
    let store_path: &str = &format!("./store.sqlite3");

    if tokio::fs::metadata(store_path).await.is_ok() {
        if let Err(e) = tokio::fs::remove_file(store_path).await {
            eprintln!("failed to remove {}: {}", store_path, e);
        }
    } else {
        println!("store not found: {}", store_path);
    }

    // Remove all files in the ./keystore directory
    match tokio::fs::read_dir(keystore_dir).await {
        Ok(mut dir) => {
            while let Ok(Some(entry)) = dir.next_entry().await {
                let file_path = entry.path();
                if let Err(e) = tokio::fs::remove_file(&file_path).await {
                    eprintln!("failed to remove {}: {}", file_path.display(), e);
                }
            }
        }
        Err(e) => eprintln!("failed to read directory {}: {}", keystore_dir, e),
    }
}
