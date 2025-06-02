use anyhow::Context; // For .context()
use miden_client::keystore::FilesystemKeyStore;
use miden_client::rpc::domain::account::AccountStorageRequirements;
use miden_client::rpc::domain::account::StorageMapKey;
use miden_client::transaction::ForeignAccount;
use miden_client::transaction::OutputNote;
use miden_client::transaction::TransactionRequestBuilder;
use miden_client::{account::AccountId};
// use miden_lib::note::utils::build_swap_tag;
use miden_lib::transaction::TransactionKernel;
// use miden_objects::assembly::{DefaultSourceManager, Library, LibraryPath, Module, ModuleKind};
use miden_objects::asset::{FungibleAsset};
use miden_objects::note::{
    Note, NoteAssets, NoteExecutionHint, NoteExecutionMode, NoteInputs, NoteMetadata,
    NoteRecipient, NoteScript, NoteTag, NoteType,
};
use miden_objects::{Felt, Word};
use miden_vm::Assembler;

#[allow(unused_imports)]
use miden_dark_pool::{
    cli::open_order::get_serial_num, // Unused in this specific test, but kept as in original
    utils::common::{client_setup, create_partial_swap_note, get_tag}, // get_tag and TestUser added for clarity
};
pub mod utils;
use utils::test_utils::{create_faucet, delete_keystore_and_store, setup_test_user};

// pub fn get_example_component_library() -> Library {
//     let source_manager = Arc::new(DefaultSourceManager::default());
//     let example_component_module = Module::parser(ModuleKind::Library)
//         .parse_str(
//             LibraryPath::new("swap_component::swap_module").unwrap(),
//             include_str!("../notes/PRIVATE_SWAPp.masm"),
//             &source_manager,
//         )
//         .unwrap();

//     TransactionKernel::assembler()
//         .with_debug_mode(true)
//         .assemble_library([example_component_module])
//         .expect("assembly should succeed")
// }

// cargo test --release --package miden-dark-pool --test swap_note -- test_swap_note --exact --show-output
#[tokio::test]
async fn test_swap_note() {
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

    let user = setup_test_user(
        &mut client,
        keystore.clone(),
        &format!("testuser"),
        faucet_a.clone(),
        faucet_b.clone(),
        100,
        20,
    )
    .await
    .account_id;

    let consumer = setup_test_user(
        &mut client,
        keystore,
        &format!("testuser2"),
        faucet_a.clone(),
        faucet_b.clone(),
        100,
        20000,
    )
    .await
    .account_id;

    let assembler: Assembler = TransactionKernel::assembler().with_debug_mode(true);

    let note_code = include_str!("../notes/PRIVATE_SWAPp.masm");
    let note_script = NoteScript::compile(note_code, assembler).unwrap();
    let note_type = NoteType::Private;

    let requested_asset_word: Word = FungibleAsset::new(faucet_b.id(), 50).unwrap().into();

    let swapp_tag = get_tag(
        note_type,
        &FungibleAsset::new(faucet_a.id(), 0).unwrap().into(),
        &FungibleAsset::new(faucet_b.id(), 0).unwrap().into(),
    )
    .unwrap();
    let p2id_tag = NoteTag::from_account_id(user.id(), NoteExecutionMode::Local).unwrap();

    let oracle_account_id = AccountId::from_hex("0x4f67e78643022e00000220d8997e33").unwrap();
    client
        .import_account_by_id(oracle_account_id)
        .await
        .unwrap();
    let oracle = client
        .get_account(oracle_account_id)
        .await
        .unwrap()
        .expect("Oracle account not found");

    let storage = oracle.account().storage();
    let publisher_count = storage
        .get_item(1)
        .context("Unable to retrieve publisher count from Oracle storage")
        .unwrap()[0]
        .as_int();

    let publisher_array: Vec<AccountId> = (1..publisher_count - 1)
        .map(|i| {
            storage
                .get_item(2 + i as u8)
                .context("Failed to retrieve publisher details")
                .map(|words| AccountId::new_unchecked([words[3], words[2]]))
        })
        .collect::<Result<_, _>>()
        .context("Failed to collect publisher array")
        .unwrap();

    let mut foreign_accounts: Vec<ForeignAccount> = vec![];

    for publisher_id in publisher_array {
        client.import_account_by_id(publisher_id).await.unwrap();
        let foreign_account = ForeignAccount::public(
            publisher_id,
            AccountStorageRequirements::new([(
                1u8,
                &[StorageMapKey::from(Word::from([
                    Felt::from(0u32),
                    Felt::from(0u32),
                    Felt::from(0u32),
                    Felt::from(120195681u32),
                ]))],
            )]),
        )
        .unwrap();
        foreign_accounts.push(foreign_account);
    }

    let oracle_foreign_account =
        ForeignAccount::public(oracle_account_id, AccountStorageRequirements::default()).unwrap();
    foreign_accounts.push(oracle_foreign_account);

    let btc_usd_pair_id: u32 = 120195681;
    let mut note_inputs_vec: Vec<Felt> = Vec::with_capacity(24); // Capacity 24 for all 24 inputs

    note_inputs_vec.extend_from_slice(&requested_asset_word);
    note_inputs_vec.push(swapp_tag.inner().into());
    note_inputs_vec.push(p2id_tag.into());
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(1));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(user.id().prefix().into());
    note_inputs_vec.push(user.id().suffix().into());
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(oracle_account_id.prefix().into());
    note_inputs_vec.push(oracle_account_id.suffix().into());
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::new(0));
    note_inputs_vec.push(Felt::from(btc_usd_pair_id));
    assert_eq!(note_inputs_vec.len(), 24, "Incorrect number of note inputs");

    let note_inputs = NoteInputs::new(note_inputs_vec).unwrap();
    let aux = Felt::new(0);
    // build the outgoing note
    let metadata = NoteMetadata::new(
        user.id(),
        note_type,
        swapp_tag,
        NoteExecutionHint::always(),
        aux,
    )
    .unwrap();
    let assets =
        NoteAssets::new(vec![FungibleAsset::new(faucet_a.id(), 50).unwrap().into()]).unwrap();
    let swap_serial_num = [Felt::new(0); 4];
    let recipient = NoteRecipient::new(swap_serial_num, note_script.clone(), note_inputs.clone());
    let note = Note::new(assets.clone(), metadata, recipient.clone());

    let note_req = TransactionRequestBuilder::new()
        .with_own_output_notes(vec![OutputNote::Full(note.clone())])
        .with_foreign_accounts(foreign_accounts.clone())
        .build()
        .unwrap();

    let tx_result = client.new_transaction(user.id(), note_req).await.unwrap();

    let _ = client.submit_transaction(tx_result).await;
    client.sync_state().await.unwrap();

    let account = client.get_account(user.id()).await.unwrap().unwrap();
    let balance1 = account
        .account()
        .vault()
        .get_balance(faucet_a.id())
        .unwrap();
    let balance2 = account
        .account()
        .vault()
        .get_balance(faucet_b.id())
        .unwrap();
    assert!(balance1 == 50);
    assert!(balance2 == 20);

    // TODO: failing with this error
    // thread 'test_swap_note' panicked at tests/swap_note.rs:168:10:
    // called `Result::unwrap()` on an `Err` value: TransactionExecutorError(TransactionProgramExecutionFailed(FailedAssertion { clk: RowIndex(3589), err_code: 131457, err_msg: Some("ID of the provided foreign account equals zero.") }))
    // note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
    // consumption of note
    // let consume_req = TransactionRequestBuilder::new()
    //     .with_unauthenticated_input_notes([(note, None)])
    //     .with_foreign_accounts(foreign_accounts)
    //     .build()
    //     .unwrap();

    // let tx_result = client
    //     .new_transaction(consumer.id(), consume_req)
    //     .await
    //     .unwrap();
    // client.submit_transaction(tx_result).await.unwrap();

    // let consumer_account = client.get_account(consumer.id()).await.unwrap().unwrap();
    // let balance1 = consumer_account
    //     .account()
    //     .vault()
    //     .get_balance(faucet_a.id())
    //     .unwrap();
    // let balance2 = consumer_account
    //     .account()
    //     .vault()
    //     .get_balance(faucet_b.id())
    //     .unwrap();
    // assert!(balance1 == 150);
    // assert!(balance2 == 20);
    delete_keystore_and_store().await;
}
