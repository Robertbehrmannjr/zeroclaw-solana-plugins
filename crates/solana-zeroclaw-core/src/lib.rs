//! `solana-zeroclaw-core`: Pure, lean, MIT-licensed Solana WASM substrate for ZeroClaw tool plugins.
//!
//! # Track E: The Shared Core (Infrastructure Prize)
//! This crate resolves the core structural blockers when compiling Solana agents for `wasm32-wasip2`:
//! 1. **Zero System Sockets (`no_std` / pure I/O)**: Replaces `solana-sdk` / `solana-client` with pure `Pubkey`, instruction builders, and versioned transaction serializers.
//! 2. **`waki` HTTP client integration**: Trait-based `RpcClient` with `MockRpcClient` for host testing (`cargo test`) and `WakiRpcClient` for live WASM `wasi:http` calls.
//! 3. **Durable Nonce Support**: Direct structural solution to **Trap 1 (Blockhash Expiry)** in async human approval workflows.
//! 4. **Token-Budget Formatting**: Pre-formatted summaries (~100-200 tokens) preventing **Trap 3 (Context Window Flooding)**.

pub mod pubkey;
pub mod instruction;
pub mod transaction;
pub mod durable_nonce;
pub mod rpc;
pub mod formatting;

pub use pubkey::Pubkey;
pub use instruction::{AccountMeta, Instruction};
pub use transaction::{VersionedMessage, VersionedTransaction};
pub use durable_nonce::DurableNonceConfig;
pub use rpc::{RpcClient, MockRpcClient, AccountInfoResponse};
#[cfg(target_family = "wasm")]
pub use rpc::WakiRpcClient;
pub use formatting::*;
