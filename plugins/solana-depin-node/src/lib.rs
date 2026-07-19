//! A ZeroClaw WIT tool plugin: `solana-depin-node`.
//!
//! # Track C: Solana DePIN Node Reference Implementation
//! Enables physical edge nodes (Raspberry Pi, ESP32, IoT gateways) running ZeroClaw to convert sensor readings into structured on-chain transactions.
//! Features:
//! - **SOP Engine Trigger Compatibility**: Works seamlessly when triggered by cron schedules, MQTT webhooks, or GPIO interrupts via ZeroClaw's SOP engine.
//! - **Deterministic Oracle Encoding**: Packs device ID, sensor discriminators, and IEEE 754 float telemetry into compact byte layouts.
//! - **Automated Micropayments / Rewards**: Optionally bundles reward payouts or data fee transfers in the exact same atomic transaction.

pub mod depin;

#[cfg(target_family = "wasm")]
mod component {
    wit_bindgen::generate!({
        path: "../../wit/v0",
        world: "tool-plugin",
        features: ["plugins-wit-v0", "plugins-wit-v0-websocket"],
    });

    use std::collections::HashMap;
    use crate::depin::{build_depin_report_transaction, DepinReportArgs};
    use exports::zeroclaw::plugin::plugin_info::Guest as PluginInfo;
    use exports::zeroclaw::plugin::tool::{Guest as Tool, ToolResult};
    use zeroclaw::plugin::logging::{
        log_record, LogLevel, PluginAction, PluginEvent, PluginOutcome,
    };

    struct SolanaDepinNode;

    const PLUGIN_NAME: &str = "solana-depin-node";
    const PLUGIN_VERSION: &str = "0.1.0";
    const TOOL_NAME: &str = "solana_depin_report";

    #[derive(serde::Deserialize)]
    struct ExecuteArgs {
        device_id: String,
        sensor_type: String,
        reading: String,
        unit: String,
        oracle_program: String,
        reporter_wallet: String,
        #[serde(default)]
        payout_recipient: Option<String>,
        #[serde(default)]
        payout_amount_sol: Option<String>,
        #[serde(default)]
        blockhash: Option<String>,
        #[serde(rename = "__config", default)]
        _config: HashMap<String, String>,
    }

    impl PluginInfo for SolanaDepinNode {
        fn plugin_name() -> String {
            PLUGIN_NAME.to_string()
        }

        fn plugin_version() -> String {
            PLUGIN_VERSION.to_string()
        }
    }

    impl Tool for SolanaDepinNode {
        fn name() -> String {
            TOOL_NAME.to_string()
        }

        fn description() -> String {
            "Compiles a Solana transaction for reporting edge device sensor telemetry (temperature, solar output, GPS, etc.) \
             to an on-chain DePIN oracle contract. Can optionally bundle reward payouts."
                .to_string()
        }

        fn parameters_schema() -> String {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "device_id": {
                        "type": "string",
                        "description": "Unique identifier of the DePIN edge node (e.g. 'RPI4-SOLAR-01')."
                    },
                    "sensor_type": {
                        "type": "string",
                        "description": "Type of sensor metric ('temperature', 'power_output', 'humidity', 'aqi')."
                    },
                    "reading": {
                        "type": "string",
                        "description": "Sensor reading value formatted as a string (e.g. '24.5')."
                    },
                    "unit": {
                        "type": "string",
                        "description": "Measurement unit ('Celsius', 'Watts', 'Percentage')."
                    },
                    "oracle_program": {
                        "type": "string",
                        "description": "Solana DePIN Oracle contract Program ID (Pubkey)."
                    },
                    "reporter_wallet": {
                        "type": "string",
                        "description": "The edge device's Solana wallet address (Pubkey)."
                    },
                    "payout_recipient": {
                        "type": "string",
                        "description": "Optional wallet address to receive automated data reporting reward or fee."
                    },
                    "payout_amount_sol": {
                        "type": "string",
                        "description": "Optional reward/fee amount in SOL (e.g. '0.01')."
                    }
                },
                "required": ["device_id", "sensor_type", "reading", "unit", "oracle_program", "reporter_wallet"]
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
                        "",
                    );
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("invalid arguments: {e}")),
                    });
                }
            };

            let depin_args = DepinReportArgs {
                device_id: parsed.device_id.clone(),
                sensor_type: parsed.sensor_type.clone(),
                reading: parsed.reading.clone(),
                unit: parsed.unit.clone(),
                oracle_program: parsed.oracle_program.clone(),
                reporter_wallet: parsed.reporter_wallet.clone(),
                payout_recipient: parsed.payout_recipient.clone(),
                payout_amount_sol: parsed.payout_amount_sol.clone(),
                blockhash: parsed.blockhash.clone(),
            };

            match build_depin_report_transaction(&depin_args) {
                Ok(res) => {
                    emit(
                        PluginAction::Complete,
                        PluginOutcome::Success,
                        &res.summary,
                        &parsed.device_id,
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
                        &format!("depin report build error: {e}"),
                        &parsed.device_id,
                    );
                    Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("depin report build error: {e}")),
                    })
                }
            }
        }
    }

    fn emit(action: PluginAction, outcome: PluginOutcome, message: &str, device_id: &str) {
        let attrs = Some(format!("{{\"device_id\":\"{device_id}\"}}"));
        log_record(
            LogLevel::Info,
            &PluginEvent {
                function_name: "solana_depin_node::tool::execute".to_string(),
                action,
                outcome: Some(outcome),
                duration_ms: None,
                attrs,
                message: message.to_string(),
            },
        );
    }

    export!(SolanaDepinNode);
}
