//! Durable Nonce handling for ZeroClaw tool plugins.
//!
//! # Solving Trap 1: The "Lunch Break" Blockhash Expiry Problem
//! When an AI agent builds a transaction that requires human sign-off (Tier 1 Custody),
//! it drops an unsigned base64 transaction or Squads proposal into a Telegram channel.
//! If the human operator is away or taking lunch, the standard 150-block (`~1 minute`)
//! `recent_blockhash` expires, making the transaction invalid when finally signed.
//!
//! By using a **Durable Nonce Account** (`AdvanceNonceAccount` system instruction + stored nonce hash),
//! the transaction remains valid indefinitely until the exact nonce is consumed on-chain.

use crate::pubkey::Pubkey;
use crate::instruction::Instruction;
use crate::transaction::{VersionedMessage, VersionedTransaction};

/// Configuration for using a Durable Nonce Account in a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableNonceConfig {
    /// The on-chain AdvanceNonce account address.
    pub nonce_account: Pubkey,
    /// The authority allowed to advance the nonce (usually the payer or session key).
    pub nonce_authority: Pubkey,
    /// The current durable blockhash stored inside the nonce account state.
    pub durable_nonce_hash: Pubkey,
}

impl DurableNonceConfig {
    pub fn new(nonce_account: Pubkey, nonce_authority: Pubkey, durable_nonce_hash: Pubkey) -> Self {
        Self {
            nonce_account,
            nonce_authority,
            durable_nonce_hash,
        }
    }

    /// Prepend the `SystemInstruction::AdvanceNonceAccount` instruction and set the message blockhash.
    pub fn apply_to_instructions(&self, mut instructions: Vec<Instruction>) -> (Vec<Instruction>, Pubkey) {
        let advance_ix = Instruction::advance_nonce_account(&self.nonce_account, &self.nonce_authority);
        // AdvanceNonceAccount MUST be the very first instruction in the transaction.
        instructions.insert(0, advance_ix);
        (instructions, self.durable_nonce_hash)
    }

    /// Compile a versioned transaction protected against blockhash expiry.
    pub fn build_durable_transaction(
        &self,
        payer: Pubkey,
        instructions: Vec<Instruction>,
    ) -> Result<VersionedTransaction, String> {
        let (durable_ixs, blockhash) = self.apply_to_instructions(instructions);
        let message = VersionedMessage::compile(payer, &durable_ixs, blockhash)?;
        Ok(VersionedTransaction::new_unsigned(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_durable_nonce_application() {
        let payer = Pubkey::system_program();
        let to = Pubkey::spl_token();
        let transfer_ix = Instruction::system_transfer(&payer, &to, 100_000);

        let nonce_account = Pubkey::memo_v2();
        let nonce_auth = payer;
        let durable_hash = Pubkey::spl_token();

        let cfg = DurableNonceConfig::new(nonce_account, nonce_auth, durable_hash);
        let (ixs, hash) = cfg.apply_to_instructions(vec![transfer_ix]);

        assert_eq!(ixs.len(), 2);
        assert_eq!(ixs[0].program_id, Pubkey::system_program());
        assert_eq!(ixs[0].data[0], 4); // AdvanceNonceAccount index
        assert_eq!(hash, durable_hash);
    }
}
