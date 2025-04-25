use bincode;
use miden_client::note::Note;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use miden_lib::utils::Deserializable;
// use miden_lib::utils::ByteReader;

#[derive(Serialize, Deserialize, Debug)]
struct MidenNote {
    id: String,
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
                    //deserialize Miden note
                    let note_bytes = note.payload;

                    //TODO: error handling should be added
                    //TODO: check for valid notes by checking the note script hash
                    let received_note: Note = Note::read_from_bytes(&note_bytes).unwrap(); 
                    println!("Received note:");
                    println!("  ID: {:?}", note.id);
                    println!("  Payload: {:#?}", received_note);
                    
                }
                Err(e) => eprintln!("Failed to deserialize note: {}", e),
            }
        });
    }
}
