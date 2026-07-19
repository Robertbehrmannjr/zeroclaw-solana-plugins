//! A ZeroClaw WIT tool plugin: `spl-transfer-build`.
//!
//! # Track A: The Transaction Compiler (Safety Prize)
//! Builds unsigned versioned transactions for SOL and SPL token transfers.
//! Specifically engineered for self-hosted human-in-the-loop approval workflows:
//! - **No Secret Key Access**: Never touches private keys or signing primitives (`config_read` only).
//! - **Durable Nonce Pre-compilation**: Optional `durable_nonce_account` argument prepends `advance_nonce_account` instruction, preventing **Trap 1 (Blockhash Expiry)** when waiting for async human approval.
//! - **Token-Optimized Summaries**: Outputs structured summary (≤ 200 tokens) alongside base64 unsigned tx, preventing **Trap 3 (Context Window Flooding)**.

pub mod transfer;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0", "plugins-wit-v0-websocket"],
    });

    use crate::transfer::{build_transfer_transaction, TransferArgs};
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };

    struct SplTransferBuild;

    const PLUGIN_NAME: &str = "spl-transfer-build";
    const PLUGIN_VERSION: &str = "0.1.0";
    const TOOL_NAME: &str = "spl_transfer_build";

    impl PluginInfo for SplTransferBuild {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for SplTransferBuild {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Builds an unsigned Solana transaction for transferring SOL or SPL tokens. \
             Returns a base64-encoded unsigned transaction alongside a concise, token-budget-friendly \
             summary suitable for Tier 1 human approval. Supports durable nonces to eliminate blockhash expiry."
                .to_string()
        }

        fn parameters_schema() -> String {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "sender": {
                        "type": "string",
                        "description": "The sender's Solana wallet address (Pubkey)."
                    },
                    "recipient": {
                        "type": "string",
                        "description": "The recipient's Solana wallet address (Pubkey)."
                    },
                    "amount": {
                        "type": "string",
                        "description": "The human-readable amount of tokens or SOL to transfer (e.g. '10.5')."
                    },
                    "mint": {
                        "type": "string",
                        "description": "The token mint address, or 'SOL' for native SOL transfer. Defaults to SOL if omitted."
                    },
                    "decimals": {
                        "type": "integer",
                        "description": "Token decimals (e.g. 6 for USDC, 9 for SOL). Defaults to 9 for SOL and 6 for SPL."
                    },
                    "blockhash": {
                        "type": "string",
                        "description": "Recent blockhash or nonce value."
                    },
                    "durable_nonce_account": {
                        "type": "string",
                        "description": "Optional durable nonce account address. If provided, prepends advance_nonce_account instruction."
                    },
                    "durable_nonce_authority": {
                        "type": "string",
                        "description": "Optional durable nonce authority address. Defaults to sender if omitted."
                    }
                },
                "required": ["sender", "recipient", "amount"]
            })
            .to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: TransferArgs = match serde_json::from_str(&args) {
                Ok(a) => a,
                Err(e) => {
                    emit(
                        PluginAction::Fail,
                        PluginOutcome::Failure,
                        &format!("invalid arguments: {e}"),
                        false,
                    );
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("invalid arguments: {e}")),
                    });
                }
            };

            match build_transfer_transaction(&parsed) {
                Ok(res) => {
                    emit(
                        PluginAction::Complete,
                        PluginOutcome::Success,
                        "built unsigned transfer transaction",
                        res.durable_nonce_used,
                    );
                    let output_json = serde_json::json!({
                        "summary": res.summary,
                        "unsigned_tx_base64": res.tx_base64,
                        "durable_nonce_used": res.durable_nonce_used
                    })
                    .to_string();

                    Ok(ToolResult {
                        success: true,
                        output: output_json,
                        error: None,
                    })
                }
                Err(e) => {
                    emit(
                        PluginAction::Fail,
                        PluginOutcome::Failure,
                        &format!("transfer build error: {e}"),
                        false,
                    );
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("transfer build error: {e}")),
                    })
                }
            }
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str, durable_nonce_used: bool) {
        let attrs = Some(format!("{{\"durable_nonce\":{durable_nonce_used}}}"));
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "spl_transfer_build::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs,
                message: message.to_string(),
            },
        );
    }

    export!(SplTransferBuild);
}
