use crate::pubkey::Pubkey;
use serde::{Deserialize, Serialize};

/// Account metadata for a Solana instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(pubkey: Pubkey, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: true,
        }
    }

    pub fn new_readonly(pubkey: Pubkey, is_signer: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable: false,
        }
    }
}

/// A Solana transaction instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}

impl Instruction {
    /// Create a generic instruction with raw bytes and account metas.
    pub fn new_with_bytes(program_id: Pubkey, data: &[u8], accounts: Vec<AccountMeta>) -> Self {
        Self {
            program_id,
            accounts,
            data: data.to_vec(),
        }
    }

    /// Create a System Program transfer instruction.
    pub fn system_transfer(from_pubkey: &Pubkey, to_pubkey: &Pubkey, lamports: u64) -> Self {
        // System instruction index 2 = transfer, followed by u64 little-endian lamports.
        let mut data = vec![2u8, 0, 0, 0];
        data.extend_from_slice(&lamports.to_le_bytes());

        Self {
            program_id: Pubkey::system_program(),
            accounts: vec![
                AccountMeta::new(*from_pubkey, true),
                AccountMeta::new(*to_pubkey, false),
            ],
            data,
        }
    }

    /// Create a System Program AdvanceNonceAccount instruction (Crucial for Trap 1: Blockhash Expiry).
    pub fn advance_nonce_account(nonce_pubkey: &Pubkey, nonce_authority: &Pubkey) -> Self {
        // System instruction index 4 = AdvanceNonceAccount.
        let data = vec![4u8, 0, 0, 0];

        Self {
            program_id: Pubkey::system_program(),
            accounts: vec![
                AccountMeta::new(*nonce_pubkey, false),
                AccountMeta::new_readonly(Pubkey::SYSVAR_RECENT_BLOCKHASH_PUBKEY, false),
                AccountMeta::new_readonly(*nonce_authority, true),
            ],
            data,
        }
    }

    /// Create an SPL Token TransferChecked instruction.
    pub fn spl_transfer_checked(
        token_program_id: &Pubkey,
        source: &Pubkey,
        mint: &Pubkey,
        destination: &Pubkey,
        authority: &Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Self {
        // TransferChecked is instruction index 12 in SPL Token v1 / Token-2022.
        let mut data = vec![12u8];
        data.extend_from_slice(&amount.to_le_bytes());
        data.push(decimals);

        Self {
            program_id: *token_program_id,
            accounts: vec![
                AccountMeta::new(*source, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(*destination, false),
                AccountMeta::new_readonly(*authority, true),
            ],
            data,
        }
    }

    /// Create an Associated Token Account idempotent creation instruction (`CreateIdempotent`).
    pub fn create_associated_token_account_idempotent(
        payer: &Pubkey,
        wallet: &Pubkey,
        mint: &Pubkey,
        token_program_id: &Pubkey,
    ) -> Self {
        let ata = Pubkey::get_associated_token_address(wallet, mint, token_program_id);
        // Instruction index 1 = CreateIdempotent (does not fail if ATA already exists).
        let data = vec![1u8];

        Self {
            program_id: Pubkey::associated_token_program(),
            accounts: vec![
                AccountMeta::new(*payer, true),
                AccountMeta::new(ata, false),
                AccountMeta::new_readonly(*wallet, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(Pubkey::system_program(), false),
                AccountMeta::new_readonly(*token_program_id, false),
            ],
            data,
        }
    }

    /// Create a Memo v2 instruction.
    pub fn memo(memo: &str) -> Self {
        Self {
            program_id: Pubkey::memo_v2(),
            accounts: vec![],
            data: memo.as_bytes().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_transfer_instruction() {
        let from = Pubkey::system_program();
        let to = Pubkey::spl_token();
        let ix = Instruction::system_transfer(&from, &to, 1_000_000);
        assert_eq!(ix.program_id, Pubkey::system_program());
        assert_eq!(ix.accounts.len(), 2);
        assert_eq!(ix.data.len(), 12);
        assert_eq!(ix.data[0], 2);
    }

    #[test]
    fn test_advance_nonce_instruction() {
        let nonce = Pubkey::system_program();
        let auth = Pubkey::spl_token();
        let ix = Instruction::advance_nonce_account(&nonce, &auth);
        assert_eq!(ix.program_id, Pubkey::system_program());
        assert_eq!(ix.accounts.len(), 3);
        assert_eq!(ix.data[0], 4);
    }
}
