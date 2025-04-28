use bincode;
use miden_client::Client;
use miden_client::ClientError;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use miden_client::account::{
    Account, AccountBuilder, AccountStorageMode, AccountType, component::BasicFungibleFaucet,
    component::BasicWallet, component::RpoFalcon512,
};
use miden_client::{
    asset::TokenSymbol, auth::AuthSecretKey, builder::ClientBuilder, crypto::SecretKey,
    keystore::FilesystemKeyStore,
};

use miden_client::rpc::{Endpoint, TonicRpcClient};
use miden_client::transaction::TransactionRequestBuilder;
use miden_lib::{note::utils::build_swap_tag, transaction::TransactionKernel, utils::Serializable};
use miden_objects::note::{
    NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata, NoteRecipient,
    NoteScript, NoteTag,
};
use miden_objects::transaction::OutputNote;
use miden_objects::{
    Felt, NoteError, Word,
    account::AccountId,
    asset::{Asset, FungibleAsset},
    note::{Note, NoteType},
};
use miden_vm::Assembler;
use rand::RngCore;
use std::sync::Arc;
use std::time::Duration;

//TODO: move MidenNote struct into common util file shared between user and matcher
// the payload vector is the serialized note
// id is the noteId
#[derive(Serialize, Deserialize, Debug)]
struct MidenNote {
    id: String,
    payload: Vec<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize client & keystore
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
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
        FilesystemKeyStore::new("./keystore".into()).unwrap();

    let sender_account = create_account(&mut client, keystore.clone()).await.unwrap();

    let faucet = create_faucet(&mut client, keystore.clone(), "MID")
        .await
        .unwrap();
    let faucet2 = create_faucet(&mut client, keystore.clone(), "ETH")
        .await
        .unwrap();

    let _ = mint_and_consume(&mut client, faucet.clone(), sender_account.clone(), 100)
        .await
        .unwrap();

    // offered asset amount
    let amount_a = 50;
    let asset_a = FungibleAsset::new(faucet.id(), amount_a).unwrap();

    // requested asset amount
    let amount_b = 50;
    let asset_b = FungibleAsset::new(faucet2.id(), amount_b).unwrap();

    // Set up the swap transaction
    let serial_num = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let fill_number = 0;

    // Create the partial swap note
    let swap_note = create_partial_swap_note(
        sender_account.id(),
        sender_account.id(),
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
        .new_transaction(sender_account.id(), note_req)
        .await
        .unwrap();

    let _ = client.submit_transaction(tx_result).await;
    client.sync_state().await?;

    // serialize the swap note for sending over tcp
    let buffer = swap_note.to_bytes();

    let note = MidenNote {
        id: swap_note.id().to_hex(),
        payload: buffer,
    };

    let encoded = bincode::serialize(&note)?;
    let length = (encoded.len() as u32).to_be_bytes();

    //TODO: right now simple tcp but encryption needs to added.
    // we can also consider some other communication protocol such as QUIC
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    stream.write_all(&length).await?;
    stream.write_all(&encoded).await?;

    println!("Note sent!");
    Ok(())
}

/// Generates a SWAP note - swap of assets between two accounts
pub fn create_partial_swap_note(
    creator: AccountId,
    last_consumer: AccountId,
    offered_asset: Asset,
    requested_asset: Asset,
    swap_serial_num: [Felt; 4],
    fill_number: u64,
) -> Result<Note, NoteError> {
    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    let note_code = include_str!("../notes/PRIVATE_SWAPp.masm");
    let note_script = NoteScript::compile(note_code, assembler).unwrap();
    let note_type = NoteType::Private;

    let requested_asset_word: Word = requested_asset.into();
    let tag = build_swap_tag(note_type, &offered_asset, &requested_asset)?;

    let swapp_tag = build_swap_tag(note_type, &offered_asset, &requested_asset)?;
    let p2id_tag = NoteTag::from_account_id(creator, NoteExecutionMode::Local)?;

    let inputs = NoteInputs::new(vec![
        requested_asset_word[0],
        requested_asset_word[1],
        requested_asset_word[2],
        requested_asset_word[3],
        swapp_tag.inner().into(),
        p2id_tag.into(),
        Felt::new(0),
        Felt::new(0),
        Felt::new(fill_number),
        Felt::new(0),
        Felt::new(0),
        Felt::new(0),
        creator.prefix().into(),
        creator.suffix().into(),
    ])?;

    let aux = Felt::new(0);

    // build the outgoing note
    let metadata = NoteMetadata::new(
        last_consumer,
        note_type,
        tag,
        NoteExecutionHint::always(),
        aux,
    )?;

    let assets = NoteAssets::new(vec![offered_asset])?;
    let recipient = NoteRecipient::new(swap_serial_num, note_script.clone(), inputs.clone());
    let note = Note::new(assets.clone(), metadata, recipient.clone());

    Ok(note)
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
