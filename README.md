# miden-dark-pool
Dark pool on Miden: A privacy-preserving dark pool trading system built on Miden, designed to allow private order submission, secure off-chain matching

⚠️ Work in Progress — This project is under development and is not production-ready. Expect breaking changes, incomplete features, and missing documentation.

## Overview
This prototype is experimenting with:

	•	Private orders via MidenNotes.
	•	Off-chain matching using a TCP-based matcher(TEE to be added).
	•	scripts for partially fillable swaps (SWAPp). The swap note script is not in its final version and some changes needs to be made.
	•	Basic CLI for submitting/cancelling orders.

## Current Status

✅ Accepts Miden notes via TCP

✅ Basic order CLI with serialization

✅ Script hash validation for SWAPp

❌ Matching engine (WIP)

❌ TEE support (planned)

❌ Secure transport (planned)


## Building, Testing and Running

- To Build: `cargo build --release`

- To Test: 
    - `cargo test --release --test user_flow -- test_open_order --exact`
    - `cargo test --release --test user_flow -- test_cancel_order --exact`

- To Run:
    - matcher: `cargo run --release --bin matcher`
    - user:
        - open-order: 
            ```sh
            cargo run --release \
            --bin user \
            -- open-order \
            --user-id <USER_ID_HEX_STRING> \
            --offered-asset <OFFERED_ASSET_HEX_ID> \
            --offered-amout <AMOUNT> \
            --requested-asset <REQUESTED_ASSET_HEX_ID> \
            --price <PRICE>
            ```
        - cancel-order:
            ```sh
            cargo run --release \
            --bin user \
            -- cancel-order \
            --user-id <USER_ID_HEX_STRING> \
            --order-id <SWAP_NOTE_HEX_ID> \
            --tag <SWAP_NOTE_TAG>
            ```
        - consume-swapped:
            ```sh
            cargo run --release \
            --bin user \
            -- consume-swapped \
            --user-id <USER_ID_HEX_STRING>
            ```
Note(Only in case of testing): in case of failure of test, delete the keystore and store
