# miden-dark-pool
A Dark pool on Miden


To run:
- Terminal 1: `cargo run --release --bin matcher`
- Terminal 2: 
```
cargo run --release --bin user \
  -- \
  --user user1 \   
  --token-a ETH \
  --amount-a 50 \
  --token-b USDC \
  --matcher-addr 127.0.0.1:8080
  ```

To Test:
`cargo test --test user_flow`