use crate::utils::common::client_setup;
use crate::utils::common::get_account;

use crate::cli::open_order::OrderError;
use clap::Parser;
use miden_client::account::AccountId;
use miden_client::transaction::TransactionRequestBuilder;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(about = "Consumes the p2id note once the swap note is fulfilled")]
pub struct ConsumeSwapped {
    /// Unique user identifier
    #[arg(long)]
    user_id: String,
}

impl ConsumeSwapped {
    pub(crate) async fn run(&self) -> Result<bool, OrderError> {
        let mut client = client_setup().await?;

        let user_id = AccountId::from_hex(&self.user_id)?;
        let user = get_account(&mut client, user_id).await.unwrap();
        let start = std::time::Instant::now();
        loop {
            // Resync to get the latest data
            client.sync_state().await?;

            let consumable_notes = client.get_consumable_notes(Some(user.id())).await?;
            let list_of_note_ids: Vec<_> =
                consumable_notes.iter().map(|(note, _)| note.id()).collect();

            if list_of_note_ids.len() > 0 {
                let transaction_request =
                    TransactionRequestBuilder::consume_notes(list_of_note_ids)
                        .build()
                        .unwrap();
                let tx_execution_result = client
                    .new_transaction(user.id(), transaction_request)
                    .await?;

                client.submit_transaction(tx_execution_result).await?;
                break;
            }

            if start.elapsed().as_secs() > 15 {
                println!("Timed out");
                return Ok(false);
            } else {
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }

        Ok(true)
    }
}
