use bincode;
use miden_objects::note::{Note, NoteTag};
use miden_objects::asset::Asset;
use miden_lib::utils::Deserializable;
use miden_tx::utils::ToHex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::collections::BTreeMap;
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

#[derive(Debug, Clone)]
struct OrderTypeError;

impl std::error::Error for OrderTypeError {}
impl std::fmt::Display for OrderTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result  {
        todo!();
    }
}

#[derive(Debug, Clone)]
struct NoteError;

impl std::error::Error for NoteError {}
impl std::fmt::Display for NoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result  {
        todo!();
    }
}

const ORDER_TYPE_BITS: u32 = 4;

enum OrderType {
    Buy = 0,
    Sell = 1,
    Cancel = 2
}

impl OrderType {
    /// Convert from u16 to OrderType
    pub fn from_u16(value: u16) -> Result<Self, OrderTypeError> {
        match value {
            0 => Ok(OrderType::Buy),
            1 => Ok(OrderType::Sell),
            2 => Ok(OrderType::Cancel),
            _ => Err(OrderTypeError),
        }
    }

    /// Convert to u16
    pub fn to_u16(self) -> u16 {
        self as u16
    }
}

struct Order {
    id: String,
    buy_asset: &Asset,
    sell_asset: &Asset,
    quantity: u128,
    price: u128,
    order_type: OrderType
}

struct OrderManager {
    buy_orders: BTreeMap<u128, Order>,
    sell_orders: BTreeMap<u128, Order>
}

impl Order {
    pub fn new(note: Note) -> Self {
        let assets = note.assets();
        assert_eq!(assets.num_assets(), 2);
        let buy_asset = assets.iter().next().unwrap();
        let buy_amount = buy_asset.unwrap_fungible().amount();
        let sell_asset = assets.iter().next().unwrap();
        let tag = &note.metadata().tag();
        let (prefix, use_case_id, payload) = decode_note_tag(tag);
        let (price, order_type) = extract_order(payload).unwrap();
        Self {
            id: note.id().to_hex(),
            buy_asset: buy_asset,
            sell_asset: sell_asset,
            quantity: buy_amount as u128,
            price: price as u128,
            order_type: order_type

        }
    }
}

pub fn decode_note_tag(nt: &NoteTag) -> (bool, u16, u16) {
    // Extract the execution bits (top 2 bits)
    let execution_bits = (nt.0 >> 30) & 0b11;
        
    // Check if this is a local use case tag (execution bits should be b11)
    let is_local_use_case = execution_bits == 0b11;
    
    // Extract the use case ID (next 14 bits)
    let use_case_id = ((nt.0 >> 16) & 0x3FFF) as u16; // 0x3FFF is 2^14 - 1
    
    // Extract the payload (bottom 16 bits)
    let payload = (nt.0 & 0xFFFF) as u16;
    
    (is_local_use_case, use_case_id, payload)
}

pub fn extract_order(payload: u16) -> Result<(u16, OrderType), OrderTypeError> {
    // Extract order type from the low bits
    let order_type_value = payload & ((1 << ORDER_TYPE_BITS) - 1);
        
    // Extract price from the high bits
    let price = payload >> ORDER_TYPE_BITS;
        
    // Convert to OrderType enum
    match OrderType::from_u16(order_type_value) {
        Ok(order_type) => Ok((price, order_type)),
        Err(e) => Err(e),
    }

    
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            buy_orders: BTreeMap::new(),
            sell_orders: BTreeMap::new(),
        }
    }

    pub async fn add_order() {

    }

    pub async fn process_orders() {

    }
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
                    let note_type = received_note.metadata().note_type();
                    let assets = received_note.assets();
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
