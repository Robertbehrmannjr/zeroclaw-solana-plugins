//! A ZeroClaw WIT tool plugin: `balance-check`.
//!
//! # Track B: Read-Only Tool / Balance Check
//! Queries native SOL or SPL token balances over pure HTTP (`wasi:http`) using `WakiRpcClient`.
//! Features:
//! - **Zero System Sockets (`no_std` / `wasi:http`)**: Operates entirely within the `wasm32-wasip2` sandbox without socket dependencies (`http_client` + `config_read` only).
//! - **Config-Driven Endpoint Selection**: Reads `rpc_url` from arguments or `__config` section, defaulting to public endpoints.
//! - **Token-Budget Formatting**: Emits compressed summaries (≤ 200 tokens) solving **Trap 3 (Context Window Flooding)**.

pub mod balance;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0", "plugins-wit-v0-websocket"],
    });

    use std::collections::HashMap;
    use crate::balance::{check_balance, BalanceArgs};
    use solana_zeroclaw_core::WakiRpcClient;
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };

    struct BalanceCheck;

    const PLUGIN_NAME: &str = "balance-check";
    const PLUGIN_VERSION: &str = "0.1.0";
    const TOOL_NAME: &str = "solana_balance";

    #[derive(serde::Deserialize)]
    struct ExecuteArgs {
        wallet: String,
        #[serde(default)]
        mint: Option<String>,
        #[serde(default)]
        decimals: Option<u8>,
        #[serde(default)]
        rpc_url: Option<String>,
        #[serde(rename = "__config", default)]
        config: HashMap<String, String>,
    }

    impl PluginInfo for BalanceCheck {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for BalanceCheck {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Checks the balance of native SOL or any SPL token for a given Solana wallet address. \
             Uses pure HTTP (wasi:http) without requiring local sockets. Outputs token-budget friendly summaries."
                .to_string()
        }

        fn parameters_schema() -> String {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "wallet": {
                        "type": "string",
                        "description": "Solana wallet address (Pubkey) to check."
                    },
                    "mint": {
                        "type": "string",
                        "description": "Optional token mint address, or 'SOL' for native balance. Defaults to SOL."
                    },
                    "decimals": {
                        "type": "integer",
                        "description": "Optional token decimals. Defaults to 9 for SOL and 6 for SPL tokens."
                    },
                    "rpc_url": {
                        "type": "string",
                        "description": "Optional custom Solana JSON-RPC URL."
                    }
                },
                "required": ["wallet"]
            })
            .to_string()
        }

        fn execute(args: String) -> Result<ToolResult, String> {
            let parsed: ExecuteArgs = match serde_json::from_str(&args) {
                Ok(a) => a,
                Err(e) => {
                    emit(
                        PluginAction::Fail,
                        PluginOutcome::Failure,
                        &format!("invalid arguments: {e}"),
                        &args,
                    );
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("invalid arguments: {e}")),
                    });
                }
            };

            let rpc_endpoint = parsed
                .rpc_url
                .or_else(|| parsed.config.get("rpc_url").cloned())
                .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());

            let client = WakiRpcClient::new(rpc_endpoint);
            let bal_args = BalanceArgs {
                wallet: parsed.wallet.clone(),
                mint: parsed.mint.clone(),
                decimals: parsed.decimals,
                rpc_url: None,
            };

            match check_balance(&client, &bal_args, 6) {
                Ok(res) => {
                    emit(
                        PluginAction::Complete,
                        PluginOutcome::Success,
                        &res.summary,
                        &parsed.wallet,
                    );
                    let out_json = serde_json::to_string(&res).unwrap_or_else(|_| res.summary.clone());
                    Ok(ToolResult {
                        success: true,
                        output: out_json,
                        error: None,
                    })
                }
                Err(e) => {
                    emit(
                        PluginAction::Fail,
                        PluginOutcome::Failure,
                        &format!("balance check error: {e}"),
                        &parsed.wallet,
                    );
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("balance check error: {e}")),
                    })
                }
            }
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str, wallet: &str) {
        let attrs = Some(format!("{{\"wallet\":\"{wallet}\"}}"));
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "balance_check::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs,
                message: message.to_string(),
            },
        );
    }

    export!(BalanceCheck);
}
