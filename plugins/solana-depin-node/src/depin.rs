use serde::{Deserialize, Serialize};
use solana_zeroclaw_core::{
    parse_human_amount_to_raw, summarize_tier1_proposal, AccountMeta, Instruction, Pubkey,
    VersionedMessage, VersionedTransaction,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepinReportArgs {
    pub device_id: String,
    pub sensor_type: String,
    pub reading: String,
    pub unit: String,
    pub oracle_program: String,
    pub reporter_wallet: String,
    pub payout_recipient: Option<String>,
    pub payout_amount_sol: Option<String>,
    pub blockhash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepinReportResult {
    pub tx_base64: String,
    pub summary: String,
    pub oracle_payload_hex: String,
}

pub fn build_depin_report_transaction(args: &DepinReportArgs) -> Result<DepinReportResult, String> {
    let oracle_pubkey = Pubkey::from_str_const_dynamic(&args.oracle_program)?;
    let reporter_pubkey = Pubkey::from_str_const_dynamic(&args.reporter_wallet)?;

    let blockhash = if let Some(bh_str) = &args.blockhash {
        Pubkey::from_str_const_dynamic(bh_str)?
    } else {
        Pubkey::from_str_const_dynamic("11111111111111111111111111111111")?
    };

    // Build the DePIN Oracle binary instruction payload
    // Layout:
    // [0..4] discriminator: 0xDE, 0x01, 0x0E, 0x00
    // [4..36] device_id hash (or zero-padded UTF-8 bytes up to 32)
    // [36..44] reading value as IEEE 754 f64 little-endian bytes
    let mut data = Vec::with_capacity(44);
    data.extend_from_slice(&[0xDE, 0x01, 0x0E, 0x00]);

    let mut device_bytes = [0u8; 32];
    let id_slice = args.device_id.as_bytes();
    let copy_len = id_slice.len().min(32);
    device_bytes[..copy_len].copy_from_slice(&id_slice[..copy_len]);
    data.extend_from_slice(&device_bytes);

    let reading_val: f64 = args
        .reading
        .parse()
        .map_err(|e| format!("Failed to parse sensor reading '{}' as f64: {}", args.reading, e))?;
    data.extend_from_slice(&reading_val.to_le_bytes());

    let mut oracle_payload_hex = String::with_capacity(data.len() * 2);
    for b in &data {
        use std::fmt::Write;
        let _ = write!(&mut oracle_payload_hex, "{:02x}", b);
    }

    let mut instructions = Vec::new();

    // Instruction 1: DePIN Oracle State Update
    let oracle_ix = Instruction::new_with_bytes(
        oracle_pubkey,
        &data,
        vec![AccountMeta::new(reporter_pubkey, true)],
    );
    instructions.push(oracle_ix);

    let mut details = Vec::new();
    details.push(("Device ID", args.device_id.as_str()));
    details.push(("Sensor Type", args.sensor_type.as_str()));
    let reading_disp = format!("{} {}", args.reading, args.unit);
    details.push(("Reading", &reading_disp));
    details.push(("Oracle Target", args.oracle_program.as_str()));

    // Instruction 2 (Optional): Automated reward payout or data transmission fee
    if let (Some(recp_str), Some(sol_amt)) = (&args.payout_recipient, &args.payout_amount_sol) {
        let recp_pubkey = Pubkey::from_str_const_dynamic(recp_str)?;
        let raw_lamports = parse_human_amount_to_raw(sol_amt, 9)?;
        let payout_ix = Instruction::system_transfer(&reporter_pubkey, &recp_pubkey, raw_lamports);
        instructions.push(payout_ix);
        details.push(("Reward Payout", sol_amt.as_str()));
    }

    let title = format!("DePIN Sensor Report ({}: {})", args.sensor_type, reading_disp);
    let message = VersionedMessage::compile(reporter_pubkey, &instructions, blockhash)?;
    let tx = VersionedTransaction::new_unsigned(message);
    let tx_base64 = tx.to_base64();

    let summary = summarize_tier1_proposal(&title, &details, &tx_base64, false);

    Ok(DepinReportResult {
        tx_base64,
        summary,
        oracle_payload_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_depin_report_transaction() {
        let args = DepinReportArgs {
            device_id: "RPI4-SOLAR-01".to_string(),
            sensor_type: "power_output".to_string(),
            reading: "1245.50".to_string(),
            unit: "Watts".to_string(),
            oracle_program: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_string(),
            reporter_wallet: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".to_string(),
            payout_recipient: Some("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr".to_string()),
            payout_amount_sol: Some("0.05".to_string()),
            blockhash: None,
        };

        let res = build_depin_report_transaction(&args).unwrap();
        assert!(!res.tx_base64.is_empty());
        assert!(res.summary.contains("DePIN Sensor Report"));
        assert!(res.summary.contains("RPI4-SOLAR-01"));
        assert_eq!(&res.oracle_payload_hex[0..8], "de010e00");
    }
}
