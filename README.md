# evm-nonce-manager

A single-wallet **EVM nonce manager** in Rust, built for systems where
several components send transactions for one wallet and a nonce collision
means a stuck or replaced transaction.

## Rules it enforces

- **Pending state is the only source of truth.** Initialisation and every
  resync read `eth_getTransactionCount(address, "pending")` — no parallel
  local counters that can drift from the chain.
- **`peek()` never advances.** A transaction can be rebuilt and re-signed
  with the same nonce.
- **`mark_accepted()` advances by exactly one**, and only after the node
  accepted the transaction (a hash was returned without error).
- **`resync()` resets to fresh chain pending** on any divergence
  (`nonce too low`, `already known`, replacement, restart).

The manager is deliberately not cloneable across components: one owner
(an orchestrator) issues nonces to everything else.

## Installation

```toml
[dependencies]
evm-nonce-manager = "0.1"
```

## Usage

```rust
use alloy::primitives::address;
use alloy::providers::ProviderBuilder;
use evm_nonce_manager::NonceManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ProviderBuilder::new()
        .connect_http("http://localhost:8545".parse()?);
    let wallet = address!("1111111111111111111111111111111111111111");

    // One manager per wallet, owned by a single orchestrator component.
    let mut mgr = NonceManager::init(&provider, wallet).await?;

    let nonce = mgr.peek();   // build & sign the transaction with this nonce
    // ... send it; ONLY after the node accepted it (a hash was returned):
    mgr.mark_accepted();

    // On any divergence ("nonce too low", "already known", a restart):
    let previous = mgr.resync_from_chain(&provider).await?;
    println!("resynced: local was {previous}, chain pending is {}", mgr.peek());
    Ok(())
}
```

Offline construction is available too: `NonceManager::from_pending(address,
pending)` builds a manager from a pending count you already have, with no
network call.

`pending_count()` is also exposed: it calls the raw JSON-RPC method so the
`pending` semantics are explicit, with no hidden provider defaults.

## Tests

```bash
cargo test
```

Unit tests cover: peek does not advance, accepted advances by exactly one
(and counts accepted transactions), resync resets to chain pending and
returns the previous value for divergence logging.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Provenance

Extracted from a production Rust trading-systems workspace and repackaged
as a standalone crate (workspace dependencies replaced with versioned
crates.io dependencies). Logic unchanged; internal references in comments
were generalised for publication. Source comments are in Ukrainian, the
author's working language.
