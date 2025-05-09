use miden_client::{
    Client, ClientError, account::Account, account::AccountId, builder::ClientBuilder,
    rpc::Endpoint, rpc::TonicRpcClient,
};

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use miden_lib::{note::utils::build_swap_tag, transaction::TransactionKernel};
use miden_objects::asset::FungibleAsset;
use miden_objects::note::{
    Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
    NoteRecipient, NoteScript, NoteTag, NoteType,
};
use miden_objects::{Felt, NoteError, Word, asset::Asset};
use miden_vm::Assembler;

// the payload vector is the serialized note
// id is the noteId
#[derive(Serialize, Deserialize, Debug)]
pub struct MidenNote {
    pub id: String,
    pub payload: Vec<u8>,
}

pub async fn client_setup() -> Result<Client, ClientError> {
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
        .with_sqlite_store("./store.sqlite3")
        .in_debug_mode(true)
        .build()
        .await?;

    let _ = client.sync_state().await?;

    Ok(client)
}

pub(crate) async fn get_account(
    client: &mut Client,
    acc_id: AccountId,
) -> Result<Account, ClientError> {
    client.import_account_by_id(acc_id).await?;

    let binding = client.get_account(acc_id).await.unwrap().unwrap();

    let account = binding.account();

    return Ok(account.clone());
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

    let note_code = include_str!("../../notes/PRIVATE_SWAPp.masm");
    let note_script = NoteScript::compile(note_code, assembler).unwrap();
    let note_type = NoteType::Private;

    let requested_asset_word: Word = requested_asset.into();

    let swapp_tag = get_tag(note_type, &offered_asset, &requested_asset)?;
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
        swapp_tag,
        NoteExecutionHint::always(),
        aux,
    )?;

    let assets = NoteAssets::new(vec![offered_asset])?;
    let recipient = NoteRecipient::new(swap_serial_num, note_script.clone(), inputs.clone());
    let note = Note::new(assets.clone(), metadata, recipient.clone());

    Ok(note)
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

/// Generates a SWAP note tag
/// build_swap_tag(note_type, asset1, asset2)
/// where asset_{i} is an Asset created with AssetId of the asset pairs and 0 amount so that the tag is deterministic for a given asset pair
fn get_tag(note_type: NoteType, asset1: &Asset, asset2: &Asset) -> Result<NoteTag, NoteError> {
    let id1 = asset1.unwrap_fungible().faucet_id();
    let id2 = asset2.unwrap_fungible().faucet_id();
    let asset1 = FungibleAsset::new(id1, 0).unwrap();
    let asset2 = FungibleAsset::new(id2, 0).unwrap();
    let tag = build_swap_tag(note_type, &asset1.into(), &asset2.into())?;
    Ok(tag)
}
