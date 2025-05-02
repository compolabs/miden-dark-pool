use bincode;
use miden_client::note::Note;
use miden_lib::utils::Deserializable;
use miden_tx::utils::ToHex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use winter_utils::Serializable;

// use miden_lib::utils::ByteReader;

//TODO: move MidenNote struct into common util file shared between user and matcher
// the payload vector is the serialized note
// id is the noteId
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
                    let note_bytes = note.payload;

                    //check for valid notes
                    //  1. check that note can be correctly deserialized
                    //  2. check that note script hash is correct
                    let received_note = Note::read_from_bytes(&note_bytes);
                    if received_note.is_err() {
                        eprintln!("Failed to deserialize note");
                    };
                    let received_note = received_note.unwrap();
                    let note_script = received_note.script().to_bytes();

                    let mut hasher = Sha256::new();
                    hasher.update(note_script);
                    let hash = hasher.finalize().to_hex();
                    println!("script hash: {:?}", hash);

                    // note script hash of PRIVATE_SWAPp note
                    if hash != "e39a29af05b233279c0009701242ff54b1d8c0d848ad2f2001eb7e0ac6ef745e" {
                        eprintln!("Not a valid note");
                    }

                    println!("Received note:");
                    println!("  ID: {:?}", note.id);
                    // println!("  Payload: {:#?}", received_note);
                }
                Err(e) => eprintln!("Failed to deserialize note: {}", e),
            }
        });
    }
}
