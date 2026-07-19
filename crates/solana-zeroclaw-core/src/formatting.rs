//! Output formatting and token-budget shaping for ZeroClaw tool plugins.
//!
//! # Solving Trap 3: Context Window Flooding
//! Raw RPC calls (`getProgramAccounts`, token balances, raw JSON quotes) can return
//! 40KB+ of JSON, destroying the LLM's context window and incurring massive API costs.
//! This module provides standardized formatting helpers that compress Solana data
//! into ≤ 200 tokens of structured, human-readable text.

/// Format raw token units into human-readable string based on decimals.
pub fn format_token_amount(raw_amount: u64, decimals: u8) -> String {
    if decimals == 0 {
        return raw_amount.to_string();
    }
    let divisor = 10u64.pow(decimals as u32);
    let whole = raw_amount / divisor;
    let frac = raw_amount % divisor;
    if frac == 0 {
        format!("{}", whole)
    } else {
        let frac_str = format!("{:0width$}", frac, width = decimals as usize);
        let trimmed_frac = frac_str.trim_end_matches('0');
        format!("{}.{}", whole, trimmed_frac)
    }
}

/// Parse human-readable token amount string into raw u64 token units.
pub fn parse_human_amount_to_raw(amount_str: &str, decimals: u8) -> Result<u64, String> {
    let clean = amount_str.trim();
    if clean.is_empty() {
        return Err("Amount string is empty".to_string());
    }
    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() > 2 {
        return Err(format!("Invalid number format: {}", clean));
    }
    let whole: u64 = parts[0]
        .parse()
        .map_err(|e| format!("Invalid whole amount '{}': {}", parts[0], e))?;
    let mut raw = whole
        .checked_mul(10u64.pow(decimals as u32))
        .ok_or_else(|| "Amount overflow".to_string())?;

    if parts.len() == 2 {
        let mut frac_str = parts[1].to_string();
        if frac_str.len() > decimals as usize {
            // Truncate extra decimal digits beyond precision
            frac_str.truncate(decimals as usize);
        } else {
            // Pad with trailing zeros to match decimal places
            while frac_str.len() < decimals as usize {
                frac_str.push('0');
            }
        }
        if !frac_str.is_empty() {
            let frac: u64 = frac_str
                .parse()
                .map_err(|e| format!("Invalid fractional amount '{}': {}", frac_str, e))?;
            raw = raw.checked_add(frac).ok_or_else(|| "Amount overflow".to_string())?;
        }
    }
    Ok(raw)
}

/// Create a token-optimized summary for a Tier 1 transaction proposal ready for human approval.
pub fn summarize_tier1_proposal(
    title: &str,
    details: &[(&str, &str)],
    tx_base64: &str,
    durable_nonce_used: bool,
) -> String {
    let mut out = format!("[Tier 1 Transaction Proposal: {}]\n", title);
    for (k, v) in details {
        out.push_str(&format!("- {}: {}\n", k, v));
    }
    out.push_str(&format!("- Blockhash Safety: {}\n", if durable_nonce_used { "Durable Nonce (Never Expires)" } else { "Standard Recent Blockhash (~1 min expiry)" }));
    out.push_str("\nBase64 Unsigned Transaction:\n");
    out.push_str(tx_base64);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_token_amount() {
        assert_eq!(format_token_amount(1_500_000, 6), "1.5");
        assert_eq!(format_token_amount(1_000_000, 6), "1");
        assert_eq!(format_token_amount(123_456_789, 9), "0.123456789");
    }

    #[test]
    fn test_parse_human_amount() {
        assert_eq!(parse_human_amount_to_raw("1.5", 6).unwrap(), 1_500_000);
        assert_eq!(parse_human_amount_to_raw("10", 6).unwrap(), 10_000_000);
        assert_eq!(parse_human_amount_to_raw("0.123456789", 9).unwrap(), 123_456_789);
    }
}
