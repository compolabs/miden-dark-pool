use bincode;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use miden_lib::transaction::TransactionKernel;
use miden_lib::utils::Serializable;

use miden_objects::{
    Felt,
    account::AccountId,
    testing::account_id::{ACCOUNT_ID_PUBLIC_FUNGIBLE_FAUCET, ACCOUNT_ID_PUBLIC_FUNGIBLE_FAUCET_1},
    asset::{Asset, FungibleAsset},
    crypto::rand::RpoRandomCoin,
    note::{Note, NoteDetails, NoteType},
};
use miden_objects::note::NoteRecipient;
use miden_objects::note::NoteMetadata;
use miden_objects::note::NoteExecutionHint;
use miden_objects::note::NoteInputs;
use miden_objects::NoteError;
use miden_vm::Assembler;
use miden_objects::note::NoteScript;
use miden_lib::note::utils::build_swap_tag;
use miden_objects::note::NoteAssets;
use miden_objects::Word;
use miden_tx::testing::{Auth, MockChain};


// use miden_crypto::utils::{ByteReader, Deserializable};

#[derive(Serialize, Deserialize, Debug)]
struct MidenNote {
    id: String,
    payload: Vec<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    
    let mut chain = MockChain::new();
    let faucet = chain.add_existing_faucet(Auth::NoAuth, "POL", 100000u64, None);
    let offered_asset = faucet.mint(100); // Offered asset to swap

    let faucet_id_2 = AccountId::try_from(ACCOUNT_ID_PUBLIC_FUNGIBLE_FAUCET_1).unwrap();
    let requested_asset: Asset = FungibleAsset::new(faucet_id_2, 100).unwrap().into(); // Requested asset for swap

    // Create accounts for sender and target
    let sender_account = chain.add_new_wallet(Auth::BasicAuth);


    // Set up the swap transaction
    let serial_num = [Felt::new(1), Felt::new(2), Felt::new(3), Felt::new(4)];
    let fill_number = 0;

    // Create the partial swap note
    let swap_note = create_partial_swap_note(
        sender_account.id(),
        sender_account.id(),
        offered_asset,
        requested_asset,
        serial_num,
        fill_number,
    )
    .unwrap();

    chain.add_pending_note(swap_note.clone());
    chain.seal_block(None, None);

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

    let note_code = include_str!("../notes/SWAPp.masm");
    let note_script = NoteScript::compile(note_code, assembler).unwrap();
    let note_type = NoteType::Private;

    let requested_asset_word: Word = requested_asset.into();
    let tag = build_swap_tag(
        note_type,
        &offered_asset,
        &requested_asset,
    )?;

    let word2: [Felt; 2] = creator.into();

    let inputs = NoteInputs::new(vec![
        requested_asset_word[0],
        requested_asset_word[1],
        requested_asset_word[2],
        requested_asset_word[3],
        tag.inner().into(),
        Felt::new(0),
        Felt::new(0),
        Felt::new(0),
        Felt::new(fill_number),
        Felt::new(0),
        Felt::new(0),
        word2[0],
        word2[1],
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
