use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::cell::RefCell;
use crate::pubkey::Pubkey;

/// JSON-RPC request format.
#[derive(Debug, Clone, Serialize)]
pub struct RpcRequest<'a> {
    pub jsonrpc: &'a str,
    pub id: u64,
    pub method: &'a str,
    pub params: Value,
}

/// JSON-RPC error format.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

/// Trait for executing JSON-RPC calls against a Solana RPC or DAS endpoint.
pub trait RpcClient {
    fn send_rpc_request(&self, method: &str, params: Value) -> Result<Value, String>;

    /// Fetch latest blockhash.
    fn get_latest_blockhash(&self) -> Result<Pubkey, String> {
        let resp = self.send_rpc_request(
            "getLatestBlockhash",
            json!([{"commitment": "confirmed"}]),
        )?;
        let blockhash_str = resp
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.get("blockhash"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Invalid getLatestBlockhash RPC response: {}", resp))?;
        Pubkey::from_str_const_dynamic(blockhash_str)
    }

    /// Fetch raw account info.
    fn get_account_info(&self, pubkey: &Pubkey) -> Result<Option<AccountInfoResponse>, String> {
        let resp = self.send_rpc_request(
            "getAccountInfo",
            json!([pubkey.to_string(), {"encoding": "base64", "commitment": "confirmed"}]),
        )?;
        let val = resp.get("result").and_then(|r| r.get("value"));
        match val {
            Some(Value::Null) | None => Ok(None),
            Some(v) => {
                let owner_str = v.get("owner").and_then(Value::as_str).unwrap_or_default();
                let owner = Pubkey::from_str_const_dynamic(owner_str)?;
                let lamports = v.get("lamports").and_then(Value::as_u64).unwrap_or(0);
                let data_array = v.get("data").and_then(Value::as_array);
                let data_base64 = if let Some(arr) = data_array {
                    arr.get(0).and_then(Value::as_str).unwrap_or_default().to_string()
                } else {
                    String::new()
                };
                let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data_base64)
                    .map_err(|e| format!("Base64 decode error for account data: {}", e))?;
                let executable = v.get("executable").and_then(Value::as_bool).unwrap_or(false);

                Ok(Some(AccountInfoResponse {
                    lamports,
                    owner,
                    data,
                    executable,
                }))
            }
        }
    }
}

impl Pubkey {
    pub fn from_str_const_dynamic(s: &str) -> Result<Self, String> {
        let bytes = bs58::decode(s)
            .into_vec()
            .map_err(|e| format!("bs58 decode error for '{}': {}", s, e))?;
        Self::from_bytes(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfoResponse {
    pub lamports: u64,
    pub owner: Pubkey,
    pub data: Vec<u8>,
    pub executable: bool,
}

/// Mock RPC Client for host-run unit tests (`cargo test`) without network.
pub struct MockRpcClient {
    pub responses: RefCell<HashMap<String, Result<Value, String>>>,
    pub default_blockhash: Pubkey,
}

impl MockRpcClient {
    pub fn new() -> Self {
        Self {
            responses: RefCell::new(HashMap::new()),
            default_blockhash: Pubkey::spl_token(),
        }
    }

    pub fn set_response(&self, method: &str, result: Value) {
        self.responses.borrow_mut().insert(method.to_string(), Ok(json!({ "result": result })));
    }

    pub fn set_error(&self, method: &str, error_msg: &str) {
        self.responses.borrow_mut().insert(method.to_string(), Err(error_msg.to_string()));
    }
}

impl RpcClient for MockRpcClient {
    fn send_rpc_request(&self, method: &str, _params: Value) -> Result<Value, String> {
        if method == "getLatestBlockhash" {
            return Ok(json!({
                "result": {
                    "value": {
                        "blockhash": self.default_blockhash.to_string()
                    }
                }
            }));
        }
        if let Some(res) = self.responses.borrow().get(method) {
            return res.clone();
        }
        Err(format!("MockRpcClient: unhandled method '{}'", method))
    }
}

/// HTTP JSON-RPC client over `wasi:http` (`waki`) for WASM components (`wasm32-wasip2`).
#[cfg(target_family = "wasm")]
pub struct WakiRpcClient {
    pub rpc_url: String,
}

#[cfg(target_family = "wasm")]
impl WakiRpcClient {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }
}

#[cfg(target_family = "wasm")]
impl RpcClient for WakiRpcClient {
    fn send_rpc_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let req_body = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method,
            params,
        };
        let body_json = serde_json::to_value(&req_body).map_err(|e| e.to_string())?;

        let resp = waki::Client::new()
            .post(&self.rpc_url)
            .header("Content-Type", "application/json")
            .json(&body_json)
            .send()
            .map_err(|e| format!("wasi:http POST to {} failed: {}", self.rpc_url, e))?
            .json::<Value>()
            .map_err(|e| format!("JSON parse error from RPC {}: {}", self.rpc_url, e))?;

        if let Some(err) = resp.get("error") {
            return Err(format!("Solana RPC Error: {}", err));
        }

        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_rpc_client() {
        let mock = MockRpcClient::new();
        let blockhash = mock.get_latest_blockhash().unwrap();
        assert_eq!(blockhash, mock.default_blockhash);

        let test_pubkey = Pubkey::system_program();
        mock.set_response(
            "getAccountInfo",
            json!({
                "value": {
                    "lamports": 1000000,
                    "owner": "11111111111111111111111111111111",
                    "data": ["", "base64"],
                    "executable": false
                }
            }),
        );

        let account = mock.get_account_info(&test_pubkey).unwrap().unwrap();
        assert_eq!(account.lamports, 1000000);
        assert_eq!(account.owner, Pubkey::system_program());
    }
}
