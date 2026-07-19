use serde::{Deserialize, Serialize};
use solana_zeroclaw_core::{format_token_amount, Pubkey, RpcClient};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceArgs {
    pub wallet: String,
    pub mint: Option<String>,
    pub decimals: Option<u8>,
    pub rpc_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceResult {
    pub wallet: String,
    pub mint: String,
    pub raw_amount: u64,
    pub formatted_amount: String,
    pub summary: String,
}

pub fn check_balance<C: RpcClient>(
    client: &C,
    args: &BalanceArgs,
    default_decimals: u8,
) -> Result<BalanceResult, String> {
    let wallet_pubkey = Pubkey::from_str_const_dynamic(&args.wallet)?;

    let is_sol = match args.mint.as_deref() {
        None | Some("SOL") | Some("11111111111111111111111111111111") => true,
        _ => false,
    };

    if is_sol {
        let decimals = args.decimals.unwrap_or(9);
        let account_opt = client.get_account_info(&wallet_pubkey)?;
        let lamports = account_opt.map(|a| a.lamports).unwrap_or(0);
        let formatted = format_token_amount(lamports, decimals);
        let summary = format!("Wallet {} has {} SOL ({} lamports)", args.wallet, formatted, lamports);

        Ok(BalanceResult {
            wallet: args.wallet.clone(),
            mint: "SOL".to_string(),
            raw_amount: lamports,
            formatted_amount: formatted,
            summary,
        })
    } else {
        let mint_str = args.mint.as_deref().unwrap();
        let mint_pubkey = Pubkey::from_str_const_dynamic(mint_str)?;
        let decimals = args.decimals.unwrap_or(default_decimals);

        let ata = Pubkey::get_associated_token_address(&wallet_pubkey, &mint_pubkey, &Pubkey::spl_token());
        let account_opt = client.get_account_info(&ata)?;

        let raw_amount = match account_opt {
            Some(acc) if acc.data.len() >= 72 => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&acc.data[64..72]);
                u64::from_le_bytes(bytes)
            }
            _ => 0,
        };

        let formatted = format_token_amount(raw_amount, decimals);
        let summary = format!("Wallet {} has {} units of token {} (ATA: {})", args.wallet, formatted, mint_str, ata);

        Ok(BalanceResult {
            wallet: args.wallet.clone(),
            mint: mint_str.to_string(),
            raw_amount,
            formatted_amount: formatted,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_zeroclaw_core::MockRpcClient;

    #[test]
    fn test_check_sol_balance() {
        let mock = MockRpcClient::new();
        let wallet = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
        let _pubkey = Pubkey::from_str_const_dynamic(wallet).unwrap();

        mock.set_response(
            "getAccountInfo",
            serde_json::json!({
                "value": {
                    "lamports": 2500000000u64,
                    "owner": "11111111111111111111111111111111",
                    "data": ["", "base64"],
                    "executable": false
                }
            }),
        );

        let args = BalanceArgs {
            wallet: wallet.to_string(),
            mint: None,
            decimals: None,
            rpc_url: None,
        };

        let res = check_balance(&mock, &args, 9).unwrap();
        assert_eq!(res.raw_amount, 2500000000);
        assert_eq!(res.formatted_amount, "2.5");
        assert!(res.summary.contains("2.5 SOL"));
    }

    #[test]
    fn test_check_spl_token_balance() {
        let mock = MockRpcClient::new();
        let wallet = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
        let mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

        // SPL token account layout is 165 bytes where offset 64..72 is u64 amount in little-endian.
        let mut data = vec![0u8; 165];
        let amount_bytes = 1500000u64.to_le_bytes(); // 1.5 USDC (6 decimals)
        data[64..72].copy_from_slice(&amount_bytes);
        let data_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);

        mock.set_response(
            "getAccountInfo",
            serde_json::json!({
                "value": {
                    "lamports": 2039280,
                    "owner": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
                    "data": [data_base64, "base64"],
                    "executable": false
                }
            }),
        );

        let args = BalanceArgs {
            wallet: wallet.to_string(),
            mint: Some(mint.to_string()),
            decimals: Some(6),
            rpc_url: None,
        };

        let res = check_balance(&mock, &args, 6).unwrap();
        assert_eq!(res.raw_amount, 1500000);
        assert_eq!(res.formatted_amount, "1.5");
        assert!(res.summary.contains("1.5 units of token"));
    }
}
