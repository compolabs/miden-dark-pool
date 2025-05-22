use bincode;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use miden_lib::utils::Serializable;

mod cli;
mod utils;
use clap::Parser;
use utils::common::MidenNote;

#[derive(Parser, Debug)]
#[command(name = "miden-cli", about = "Dark pool CLI")]
pub enum Cli {
    #[command(name = "open-order")]
    OpenOrder(cli::open_order::OpenOrder),

    #[command(name = "cancel-order")]
    CancelOrder(cli::cancel_order::CancelOrder),

    #[command(name = "consume-swapped")]
    ConsumeSwapped(cli::consume_swapped::ConsumeSwapped),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli {
        Cli::OpenOrder(cmd) => {
            let swap_note = cmd.run().await?;
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

            println!("Note sent");
            println!("Note id: {}", swap_note.id().to_hex());
        }

        Cli::CancelOrder(cmd) => {
            let result = cmd.run().await?;
            println!("{}", result);
        }

        Cli::ConsumeSwapped(cmd) => {
            let result = cmd.run().await?;
            println!("{}", result);
        }
    }

    Ok(())
}
