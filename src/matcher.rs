use bincode;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

#[derive(Serialize, Deserialize, Debug)]
struct MidenNote {
    id: u64,
    payload: Vec<u8>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Matcher listening on 127.0.0.1:8080");

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut len_buf = [0u8; 4];
            if socket.read_exact(&mut len_buf).await.is_err() {
                eprintln!("Failed to read length");
                return;
            }

            let len = u32::from_be_bytes(len_buf) as usize;
            let mut buffer = vec![0u8; len];

            if socket.read_exact(&mut buffer).await.is_err() {
                eprintln!("Failed to read payload");
                return;
            }

            match bincode::deserialize::<MidenNote>(&buffer) {
                Ok(note) => {
                    let payload_str = String::from_utf8(note.payload.clone())
                        .unwrap_or("[invalid UTF-8]".to_string());
                    println!("Received note:");
                    println!("  ID: {}", note.id);
                    println!("  Payload: {}", payload_str);
                },
                Err(e) => eprintln!("Failed to deserialize note: {}", e),
            }
        });
    }
}
