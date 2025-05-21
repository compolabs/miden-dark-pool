# miden-dark-pool
Dark pool on Miden: A privacy-preserving dark pool trading system built on Miden, designed to allow private order submission, secure off-chain matching

⚠️ Work in Progress — This project is under development and is not production-ready. Expect breaking changes, incomplete features, and missing documentation.

## Overview
This prototype is experimenting with:

	•	Private orders via MidenNotes
	•	Off-chain matching using a TCP-based matcher(TEE to be added)
	•	scripts for partially fillable swaps (SWAPp)
	•	Basic CLI for submitting/cancelling orders

## Current Status

✅ Accepts Miden notes via TCP

✅ Basic order CLI with serialization

✅ Script hash validation for SWAPp

❌ Matching engine (WIP)

❌ TEE support (planned)

❌ Secure transport (planned)


## Building and Testing

- To Build: `cargo build --release`

- To Test: 
    - `cargo test --release --test user_flow -- test_open_order --exact`
    - `cargo test --release --test user_flow -- test_cancel_order --exact`

Note: in case of failure of test, delete the keystore and store