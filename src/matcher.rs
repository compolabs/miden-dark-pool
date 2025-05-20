use bincode;
use miden_client::Felt;
use miden_lib::utils::Deserializable;
use miden_objects::Word;
use miden_objects::asset::Asset;
use miden_objects::note::{Note, NoteTag};
use miden_tx::utils::ToHex;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use winter_utils::Serializable;
// use std::sync::Arc;
// use std::sync::Mutex;
// use miden_client::Client;
// use tokio::net::TcpStream;

pub mod utils;
use utils::common::MidenNote;

#[derive(Debug, Clone)]
pub struct OrderTypeError;

impl std::error::Error for OrderTypeError {}
impl std::fmt::Display for OrderTypeError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!();
    }
}

#[derive(Debug, Clone)]
struct NoteError;

impl std::error::Error for NoteError {}
impl std::fmt::Display for NoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!();
    }
}

const ORDER_TYPE_BITS: u32 = 4;

#[derive(Debug, Clone)]
pub enum OrderType {
    Buy = 0,
    Sell = 1,
    Cancel = 2,
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

    #[allow(dead_code)]
    /// Convert to u16
    pub fn to_u16(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub buy_id: String,
    pub sell_id: String,
    pub price: u128,
    pub quantity: u128,
}

#[derive(Debug, Clone)]
pub struct OrderOutcome {
    pub side: String,
    pub filled_qty: u128,
    pub remaining_qty: u128,
    pub execution_price: Option<u128>,
    pub status: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Order {
    id: String,
    buy_asset: Asset,
    sell_asset: Asset,
    quantity: u128,
    price: u128,
    order_type: OrderType,
}

#[allow(dead_code)]
struct OrderManager {
    buy_orders: BTreeMap<u128, VecDeque<Order>>,
    sell_orders: BTreeMap<u128, VecDeque<Order>>,
}

impl Order {
    #[allow(dead_code)]
    pub fn new(note: Note) -> Self {
        let assets = note.assets();
        assert_eq!(assets.num_assets(), 1);
        let offered_asset = assets.iter().next().unwrap();
        let offered_amount = offered_asset.unwrap_fungible().amount();
        // let sell_asset = assets.iter().next().unwrap();
        let tag = &note.metadata().tag();
        let req_asset_felt: [Felt; 4] =
            note.recipient().inputs().values()[0..4].try_into().unwrap();
        let req_asset = Asset::try_from(Word::from(req_asset_felt)).unwrap();
        let req_amount = req_asset.unwrap_fungible().amount();
        let price = req_amount / offered_amount;
        let (_, _, payload) = decode_note_tag(tag);
        let (_, order_type) = extract_order(payload).unwrap();
        Self {
            id: note.id().to_hex(),
            buy_asset: req_asset,
            sell_asset: *offered_asset,
            quantity: offered_amount as u128,
            price: price as u128,
            order_type: order_type,
        }
    }
}

pub fn decode_note_tag(nt: &NoteTag) -> (bool, u16, u16) {
    // Extract the execution bits (top 2 bits)
    let execution_bits = (nt.inner() >> 30) & 0b11;

    // Check if this is a local use case tag (execution bits should be b11)
    let is_local_use_case = execution_bits == 0b11;

    // Extract the use case ID (next 14 bits)
    let use_case_id = ((nt.inner() >> 16) & 0x3FFF) as u16; // 0x3FFF is 2^14 - 1

    // Extract the payload (bottom 16 bits)
    let payload = (nt.inner() & 0xFFFF) as u16;

    (is_local_use_case, use_case_id, payload)
}

#[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            buy_orders: BTreeMap::new(),
            sell_orders: BTreeMap::new(),
        }
    }

    #[allow(dead_code)]
    pub async fn add_order(&mut self, note: Note) -> Result<(), OrderTypeError> {
        let order = Order::new(note);
        match order.order_type {
            OrderType::Buy => {
                self.buy_orders
                    .entry(order.price)
                    .or_insert_with(VecDeque::new)
                    .push_back(order);
            }
            OrderType::Sell => {
                self.sell_orders
                    .entry(order.price)
                    .or_insert_with(VecDeque::new)
                    .push_back(order);
            }
            OrderType::Cancel => {
                // self.buy_orders.retain(|_, o| o.id != order.id);
                // self.sell_orders.retain(|_, o| o.id != order.id);
                todo!()
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn process_orders(
        &mut self,
        cancellations: Vec<String>,
        external_midpoint: u128,
        randomize: bool,
    ) -> (Vec<MatchResult>, HashMap<String, OrderOutcome>) {
        let cancelled_ids: std::collections::HashSet<_> = cancellations.into_iter().collect();

        let active_orders: Vec<Order> = self
            .buy_orders
            .values_mut()
            .flat_map(|q| q.drain(..))
            .chain(self.sell_orders.values_mut().flat_map(|q| q.drain(..)))
            .filter(|o| !cancelled_ids.contains(&o.id))
            .collect();

        let mut eligible_buys: Vec<Order> = active_orders
            .iter()
            .filter(|o| matches!(o.order_type, OrderType::Buy) && o.price >= external_midpoint)
            .cloned()
            .collect();

        let mut eligible_sells: Vec<Order> = active_orders
            .iter()
            .filter(|o| matches!(o.order_type, OrderType::Sell) && o.price <= external_midpoint)
            .cloned()
            .collect();

        if eligible_buys.is_empty() || eligible_sells.is_empty() {
            return (vec![], HashMap::new());
        }

        if randomize {
            use rand::seq::SliceRandom;
            let mut rng = rand::rng();
            eligible_buys.shuffle(&mut rng);
            eligible_sells.shuffle(&mut rng);
        } else {
            eligible_buys.sort_by_key(|o| o.id.clone()); // FIFO by ID or timestamp if available
            eligible_sells.sort_by_key(|o| o.id.clone());
        }

        let mut matches = vec![];
        let mut outcomes = HashMap::new();

        let mut i = 0;
        let mut j = 0;
        while i < eligible_buys.len() && j < eligible_sells.len() {
            let buy = eligible_buys[i].clone();
            let sell = eligible_sells[j].clone();

            let match_qty = buy.quantity.min(sell.quantity);

            // Record match
            matches.push(MatchResult {
                buy_id: buy.id.clone(),
                sell_id: sell.id.clone(),
                price: external_midpoint,
                quantity: match_qty,
            });

            // Update and store outcomes
            outcomes.insert(
                buy.id.clone(),
                OrderOutcome {
                    side: "buy".to_string(),
                    filled_qty: match_qty,
                    remaining_qty: buy.quantity - match_qty,
                    execution_price: Some(external_midpoint),
                    status: if buy.quantity == match_qty {
                        "filled"
                    } else {
                        "partial"
                    }
                    .to_string(),
                },
            );

            outcomes.insert(
                sell.id.clone(),
                OrderOutcome {
                    side: "sell".to_string(),
                    filled_qty: match_qty,
                    remaining_qty: sell.quantity - match_qty,
                    execution_price: Some(external_midpoint),
                    status: if sell.quantity == match_qty {
                        "filled"
                    } else {
                        "partial"
                    }
                    .to_string(),
                },
            );

            // Adjust pointers
            if buy.quantity == match_qty {
                i += 1;
            } else {
                eligible_buys[i].quantity -= match_qty;
            }

            if sell.quantity == match_qty {
                j += 1;
            } else {
                eligible_sells[j].quantity -= match_qty;
            }
        }

        // Step 5: Mark unfilled
        for o in active_orders {
            if !outcomes.contains_key(&o.id) {
                outcomes.insert(
                    o.id.clone(),
                    OrderOutcome {
                        side: match o.order_type {
                            OrderType::Buy => "buy".to_string(),
                            OrderType::Sell => "sell".to_string(),
                            OrderType::Cancel => continue,
                        },
                        filled_qty: 0,
                        remaining_qty: o.quantity,
                        execution_price: None,
                        status: "unfilled".to_string(),
                    },
                );
            }
        }

        (matches, outcomes)
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
                    //  3. check that note is submitted to the network
                    //  4. check that note is consumable
                    let received_note = Note::read_from_bytes(&note_bytes);
                    if received_note.is_err() {
                        eprintln!("Failed to deserialize note");
                    };
                    let received_note = received_note.unwrap();
                    let _note_type = received_note.metadata().note_type();
                    let _assets = received_note.assets();
                    let _tag = received_note.metadata().tag();

                    let note_script = received_note.script().to_bytes();
                    // let network_note = client.get_input_note(received_note.id()).await.unwrap().unwrap();
                    let mut hasher = Sha256::new();
                    hasher.update(note_script);
                    let hash = hasher.finalize().to_hex();
                    println!("script hash: {:?}", hash);

                    // let network_note = client.get_input_note(received_note.id()).await.unwrap().unwrap();
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
