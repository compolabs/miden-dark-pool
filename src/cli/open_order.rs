use crate::utils::common::client_setup;
use crate::utils::common::create_partial_swap_note;
use crate::utils::common::get_account;

use clap::Parser;
use miden_client::ClientError;
use miden_client::account::AccountId;
use miden_client::note::Note;
use miden_client::transaction::TransactionRequestBuilder;
use miden_objects::AccountIdError;
use miden_objects::Felt;
use miden_objects::asset::FungibleAsset;
use miden_objects::transaction::OutputNote;
use rand::Rng;
use sha2::Digest;
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(about = "Opens a new order")]
pub struct OpenOrder {
    /// Unique user identifier
    #[arg(long)]
    user_id: String,

    /// Token user is offering (e.g. ETH)
    #[arg(long)]
    offered_asset: String,

    /// Amount of token_a
    #[arg(long)]
    offered_amount: u64,

    /// Token user wants in return (e.g. USDC)
    #[arg(long)]
    requested_asset: String,

    /// Price
    #[arg(long)]
    price: u64,
}

#[derive(Error, Debug)]
pub enum OpenOrderError {
    #[error("Client error:")]
    Client(#[from] ClientError),

    #[error("Account error:")]
    InvalidAccountID(#[from] AccountIdError),
}

impl OpenOrder {
    pub(crate) async fn run(&self) -> Result<Note, OpenOrderError> {
        let mut client = client_setup().await?;

        let user_id = AccountId::from_hex(&self.user_id)?;
        let user = get_account(&mut client, user_id).await.unwrap();

        let offered_asset_id = AccountId::from_hex(&self.offered_asset)?;
        let offered_asset = get_account(&mut client, offered_asset_id).await?;

        let requested_asset_id = AccountId::from_hex(&self.requested_asset)?;
        let requested_asset = get_account(&mut client, requested_asset_id).await?;

        // offered asset amount
        let asset_a = FungibleAsset::new(offered_asset.id(), self.offered_amount).unwrap();

        //TODO: requested asset amount should be based on the price
        // requested asset amount
        let asset_b = FungibleAsset::new(requested_asset.id(), self.offered_amount).unwrap();

        // Set up the swap transaction
        let serial_num = get_serial_num(user_id);
        let fill_number = 0;

        // Create the partial swap note
        let swap_note = create_partial_swap_note(
            user.id(),
            user.id(),
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

        let tx_result = client.new_transaction(user.id(), note_req).await.unwrap();

        let _ = client.submit_transaction(tx_result).await;
        client.sync_state().await?;

        Ok(swap_note)
    }
}

/// Generates a random serial number
/// hash(AccountId||random u64)
/// AccountId is treated as domain separation tag
fn get_serial_num(acc_id: AccountId) -> [Felt; 4] {
    let mut rng = rand::rng();
    let num = rng.r#random::<u64>();
    let data = format!("{}{}", acc_id.to_hex(), num);
    let hash: [u8; 32] = sha2::Sha256::digest(&data.as_bytes()).into();

    let serial_num: [Felt; 4] = [
        Felt::new(u64::from_be_bytes(hash[0..8].try_into().unwrap())),
        Felt::new(u64::from_be_bytes(hash[8..16].try_into().unwrap())),
        Felt::new(u64::from_be_bytes(hash[16..24].try_into().unwrap())),
        Felt::new(u64::from_be_bytes(hash[24..32].try_into().unwrap())),
    ];
    serial_num
}
