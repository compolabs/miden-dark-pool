use bincode;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use miden_client::{
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, TonicRpcClient},
    transaction::TransactionRequestBuilder,
};

use miden_lib::{note::utils::build_swap_tag, transaction::TransactionKernel, utils::Serializable};
use miden_objects::note::{
    Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
    NoteRecipient, NoteScript, NoteTag, NoteType,
};
use miden_objects::transaction::OutputNote;
use miden_objects::{
    Felt, NoteError, Word,
    account::AccountId,
    asset::{Asset, FungibleAsset},
};
use miden_vm::Assembler;
use std::sync::Arc;

mod utils;
use clap::Parser;
use std::net::SocketAddr;
use thiserror::Error;
use utils::utility::{create_faucet, mint_and_consume};
use utils::{
    common::{MidenNote, delete_keystore_and_store},
    utility::create_account,
};

#[derive(Error, Debug)]
pub enum UserError {
    #[error("Unable to connect to client")]
    ClientNotAbleToConnect,
}

#[derive(Parser, Debug)]
#[command(about = "Submit a swap Note(private) to the matcher")]
struct Cli {
    /// Unique user identifier
    #[arg(long)]
    user: String,

    /// Token user is offering (e.g. ETH)
    #[arg(long)]
    token_a: String,

    /// Amount of token_a
    #[arg(long)]
    amount_a: u64,

    /// Token user wants in return (e.g. USDC)
    #[arg(long)]
    token_b: String,

    /// Matcher TCP address (e.g. 127.0.0.1:8080)
    #[arg(long)]
    matcher_addr: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    assert!(cli.token_a != cli.token_b);
    assert!(cli.amount_a > 0);

    let keystore_path = format!("./keystore");

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
        .with_filesystem_keystore(&keystore_path)
        .in_debug_mode(true)
        .build()
        .await?;

    let sync_summary = client.sync_state().await.unwrap();
    println!("Latest block: {}", sync_summary.block_num);

    // let keystore: FilesystemKeyStore<rand::prelude::StdRng> =
    //     FilesystemKeyStore::new(keystore_path.into()).unwrap();

    // let sender_account = create_account(&mut client, keystore.clone()).await.unwrap();

    client
        .import_account_by_id(AccountId::from_hex(&cli.user).unwrap())
        .await
        .unwrap();

    let binding = client
        .get_account(AccountId::from_hex(&cli.user).unwrap())
        .await
        .unwrap()
        .unwrap();

    let sender_account = binding.account();

    client
        .import_account_by_id(AccountId::from_hex(&cli.token_a).unwrap())
        .await
        .unwrap();
    let binding = client
        .get_account(AccountId::from_hex(&cli.token_a).unwrap())
        .await
        .unwrap()
        .unwrap();
    let faucet = binding.account();

    client
        .import_account_by_id(AccountId::from_hex(&cli.token_b).unwrap())
        .await?;
    let binding = client
        .get_account(AccountId::from_hex(&cli.token_b).unwrap())
        .await
        .unwrap()
        .unwrap();
    let faucet2 = binding.account();

    // let faucet2 = create_faucet(&mut client, keystore.clone(), "ETH")
    //     .await
    //     .unwrap();

    // let _ = mint_and_consume(&mut client, faucet.clone(), sender_account.clone(), 1000)
    //     .await
    //     .unwrap();

    // offered asset amount
    let amount_a = cli.amount_a;
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

    // This is added so that the user.rs can be called multiple times with different sqlite3 file
    delete_keystore_and_store(&cli.user).await;

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
