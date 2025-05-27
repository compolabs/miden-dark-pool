use crate::cli::open_order::OrderError;
use crate::utils::common::{client_setup, get_account};

use clap::Parser;
use miden_client::account::AccountId;
use miden_client::note::{NoteId, NoteTag};
use miden_client::transaction::TransactionRequestBuilder;

#[derive(Parser, Debug)]
#[command(about = "Cancel an existing order")]
pub struct CancelOrder {
    /// Unique user identifier
    #[arg(long)]
    user_id: String,

    /// OrderId
    #[arg(long)]
    order_id: String,

    /// Order tag
    #[arg(long)]
    tag: u32,
}

impl CancelOrder {
    pub(crate) async fn run(&self) -> Result<bool, OrderError> {
        let mut client = client_setup().await?;

        let account_id = AccountId::from_hex(&self.user_id)?;
        let user = get_account(&mut client, account_id).await?;

        let order_id = NoteId::try_from_hex(self.order_id.as_str())?;

        let _note_tag = NoteTag::from(self.tag);

        let consumable_notes = client.get_input_note(order_id).await?.unwrap();
        let id = consumable_notes.id();
        if consumable_notes.is_consumed() {
            return Err(OrderError::OrderAlreadyConsumed);
        }

        let transaction_request = TransactionRequestBuilder::consume_notes(vec![id])
            .build()
            .unwrap();
        let tx_execution_result = client
            .new_transaction(user.id(), transaction_request)
            .await?;

        client.submit_transaction(tx_execution_result).await?;
        let _ = client.sync_state().await?;
        println!("note consumed successfully.");
        println!("Order cancelled");
        Ok(true)
    }
}
