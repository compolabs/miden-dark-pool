use bincode;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use miden_objects::note::Note;

#[derive(Serialize, Deserialize, Debug)]
struct MidenNote {
    id: u64,
    payload: Vec<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let note = MidenNote {
        id: 42,
        payload: b"plain data".to_vec(), // to be replaced by miden notes.
    };

    let encoded = bincode::serialize(&note)?;
    let length = (encoded.len() as u32).to_be_bytes();

    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    stream.write_all(&length).await?;
    stream.write_all(&encoded).await?;

    println!("Note sent!");
    Ok(())
}
