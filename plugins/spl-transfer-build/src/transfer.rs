use serde::{Deserialize, Serialize};
use solana_zeroclaw_core::{
    parse_human_amount_to_raw, summarize_tier1_proposal, Instruction, Pubkey, VersionedMessage,
    VersionedTransaction,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferArgs {
    pub sender: String,
    pub recipient: String,
    pub amount: String,
    pub mint: Option<String>,
    pub decimals: Option<u8>,
    pub blockhash: Option<String>,
    pub durable_nonce_account: Option<String>,
    pub durable_nonce_authority: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferBuildResult {
    pub tx_base64: String,
    pub summary: String,
    pub durable_nonce_used: bool,
}

pub fn build_transfer_transaction(args: &TransferArgs) -> Result<TransferBuildResult, String> {
    let sender_pubkey = Pubkey::from_str_const_dynamic(&args.sender)?;
    let recipient_pubkey = Pubkey::from_str_const_dynamic(&args.recipient)?;

    let is_sol = match args.mint.as_deref() {
        None | Some("SOL") | Some("11111111111111111111111111111111") => true,
        _ => false,
    };

    let decimals = args.decimals.unwrap_or(if is_sol { 9 } else { 6 });
    let raw_amount = parse_human_amount_to_raw(&args.amount, decimals)?;

    let mut instructions = Vec::new();
    let mut durable_nonce_used = false;

    // Check if durable nonce is requested (Trap 1 solution)
    let blockhash_or_nonce = if let Some(nonce_acc_str) = &args.durable_nonce_account {
        let nonce_pubkey = Pubkey::from_str_const_dynamic(nonce_acc_str)?;
        let nonce_auth_str = args
            .durable_nonce_authority
            .as_deref()
            .unwrap_or(&args.sender);
        let nonce_auth_pubkey = Pubkey::from_str_const_dynamic(nonce_auth_str)?;

        let advance_ix = Instruction::advance_nonce_account(&nonce_pubkey, &nonce_auth_pubkey);
        instructions.push(advance_ix);
        durable_nonce_used = true;

        // If blockhash is passed, we use it as the nonce value; otherwise fall back to token program id
        if let Some(bh_str) = &args.blockhash {
            Pubkey::from_str_const_dynamic(bh_str)?
        } else {
            Pubkey::spl_token()
        }
    } else {
        let bh_str = args
            .blockhash
            .as_deref()
            .unwrap_or("11111111111111111111111111111111");
        Pubkey::from_str_const_dynamic(bh_str)?
    };

    let details_title;
    let mut details = Vec::new();
    details.push(("Sender", args.sender.as_str()));
    details.push(("Recipient", args.recipient.as_str()));
    details.push(("Amount", args.amount.as_str()));

    if is_sol {
        details_title = format!("SOL Transfer ({} SOL)", args.amount);
        let transfer_ix = Instruction::system_transfer(&sender_pubkey, &recipient_pubkey, raw_amount);
        instructions.push(transfer_ix);
    } else {
        let mint_str = args.mint.as_deref().unwrap();
        let mint_pubkey = Pubkey::from_str_const_dynamic(mint_str)?;
        details_title = format!("SPL Token Transfer ({} units of {})", args.amount, mint_str);

        let sender_ata = Pubkey::get_associated_token_address(&sender_pubkey, &mint_pubkey, &Pubkey::spl_token());
        let recipient_ata = Pubkey::get_associated_token_address(&recipient_pubkey, &mint_pubkey, &Pubkey::spl_token());

        let transfer_ix = Instruction::spl_transfer_checked(
            &Pubkey::spl_token(),
            &sender_ata,
            &mint_pubkey,
            &recipient_ata,
            &sender_pubkey,
            raw_amount,
            decimals,
        );
        instructions.push(transfer_ix);
        details.push(("Token Mint", mint_str));
    }

    let message = VersionedMessage::compile(sender_pubkey, &instructions, blockhash_or_nonce)?;
    let tx = VersionedTransaction::new_unsigned(message);
    let tx_base64 = tx.to_base64();

    let summary = summarize_tier1_proposal(&details_title, &details, &tx_base64, durable_nonce_used);

    Ok(TransferBuildResult {
        tx_base64,
        summary,
        durable_nonce_used,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_sol_transfer() {
        let args = TransferArgs {
            sender: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            recipient: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(),
            amount: "1.5".to_string(),
            mint: Some("SOL".to_string()),
            decimals: Some(9),
            blockhash: Some("11111111111111111111111111111111".to_string()),
            durable_nonce_account: None,
            durable_nonce_authority: None,
        };

        let res = build_transfer_transaction(&args).unwrap();
        assert!(!res.tx_base64.is_empty());
        assert!(res.summary.contains("SOL Transfer"));
        assert!(!res.durable_nonce_used);
    }

    #[test]
    fn test_build_spl_transfer_with_durable_nonce() {
        let args = TransferArgs {
            sender: "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM".to_string(),
            recipient: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(),
            amount: "100".to_string(),
            mint: Some("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string()),
            decimals: Some(6),
            blockhash: Some("11111111111111111111111111111111".to_string()),
            durable_nonce_account: Some("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr".to_string()),
            durable_nonce_authority: None,
        };

        let res = build_transfer_transaction(&args).unwrap();
        assert!(res.durable_nonce_used);
        assert!(res.summary.contains("Durable Nonce (Never Expires)"));
    }
}
