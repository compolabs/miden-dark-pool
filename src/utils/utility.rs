use miden_client::Client;
use miden_client::ClientError;
use miden_client::account::{
    Account, AccountBuilder, AccountStorageMode, AccountType, component::BasicFungibleFaucet,
    component::BasicWallet, component::RpoFalcon512,
};

#[allow(unused_imports)]
use miden_objects::note::{
    Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
    NoteRecipient, NoteScript, NoteTag, NoteType,
};

#[allow(unused_imports)]
use miden_objects::{
    Felt, NoteError, Word,
    account::AccountId,
    asset::{Asset, FungibleAsset},
};

#[allow(unused_imports)]
use miden_client::{
    asset::TokenSymbol,
    auth::AuthSecretKey,
    builder::ClientBuilder,
    crypto::SecretKey,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
    transaction::TransactionRequestBuilder,
};

use rand::RngCore;
use std::time::Duration;

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
    tokio::time::sleep(Duration::from_secs(2)).await;

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

    println!("tx request built");

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
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(())
}
