use crate::cli::open_order::OrderError;
use clap::Parser;

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
    tag: String,
}

impl CancelOrder {
    pub(crate) async fn run(&self) -> Result<bool, OrderError> {
        todo!()
    }
}
