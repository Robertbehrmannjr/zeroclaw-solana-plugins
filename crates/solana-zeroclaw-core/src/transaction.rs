use crate::pubkey::Pubkey;
use crate::instruction::Instruction;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::{Deserialize, Serialize};

/// Compiled message header specifying the number of signers and read-only accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MessageHeader {
    pub num_required_signatures: u8,
    pub num_readonly_signed_accounts: u8,
    pub num_readonly_unsigned_accounts: u8,
}

/// Compiled instruction for wire format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

/// Address Table Lookup for versioned transactions (v0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageAddressTableLookup {
    pub account_key: Pubkey,
    pub writable_indexes: Vec<u8>,
    pub readonly_indexes: Vec<u8>,
}

/// Versioned Message (v0 / legacy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedMessage {
    pub header: MessageHeader,
    pub account_keys: Vec<Pubkey>,
    pub recent_blockhash: Pubkey,
    pub instructions: Vec<CompiledInstruction>,
    pub address_table_lookups: Vec<MessageAddressTableLookup>,
}

impl VersionedMessage {
    /// Compile a list of instructions into a clean VersionedMessage (v0/legacy hybrid).
    pub fn compile(
        payer: Pubkey,
        instructions: &[Instruction],
        recent_blockhash: Pubkey,
    ) -> Result<Self, String> {
        let mut account_keys = Vec::new();
        let mut signers_writable = Vec::new();
        let mut signers_readonly = Vec::new();
        let mut nonsigners_writable = Vec::new();
        let mut nonsigners_readonly = Vec::new();

        // Payer is always first signer & writable.
        signers_writable.push(payer);

        // Collect all unique pubkeys across instructions.
        for ix in instructions {
            for meta in &ix.accounts {
                if meta.pubkey == payer {
                    continue;
                }
                if meta.is_signer {
                    if meta.is_writable {
                        if !signers_writable.contains(&meta.pubkey) {
                            signers_writable.push(meta.pubkey);
                        }
                    } else if !signers_readonly.contains(&meta.pubkey) && !signers_writable.contains(&meta.pubkey) {
                        signers_readonly.push(meta.pubkey);
                    }
                } else if meta.is_writable {
                    if !nonsigners_writable.contains(&meta.pubkey)
                        && !signers_writable.contains(&meta.pubkey)
                        && !signers_readonly.contains(&meta.pubkey)
                    {
                        nonsigners_writable.push(meta.pubkey);
                    }
                } else if !nonsigners_readonly.contains(&meta.pubkey)
                    && !nonsigners_writable.contains(&meta.pubkey)
                    && !signers_writable.contains(&meta.pubkey)
                    && !signers_readonly.contains(&meta.pubkey)
                {
                    nonsigners_readonly.push(meta.pubkey);
                }
            }
            // Ensure program_id is included as readonly nonsigner if not already present.
            if ix.program_id != payer
                && !signers_writable.contains(&ix.program_id)
                && !signers_readonly.contains(&ix.program_id)
                && !nonsigners_writable.contains(&ix.program_id)
                && !nonsigners_readonly.contains(&ix.program_id)
            {
                nonsigners_readonly.push(ix.program_id);
            }
        }

        account_keys.extend(signers_writable.iter().cloned());
        account_keys.extend(signers_readonly.iter().cloned());
        account_keys.extend(nonsigners_writable.iter().cloned());
        account_keys.extend(nonsigners_readonly.iter().cloned());

        let header = MessageHeader {
            num_required_signatures: (signers_writable.len() + signers_readonly.len()) as u8,
            num_readonly_signed_accounts: signers_readonly.len() as u8,
            num_readonly_unsigned_accounts: nonsigners_readonly.len() as u8,
        };

        let mut compiled_instructions = Vec::new();
        for ix in instructions {
            let program_id_index = account_keys
                .iter()
                .position(|k| *k == ix.program_id)
                .ok_or_else(|| format!("Program id {} not found in account_keys", ix.program_id))?
                as u8;

            let mut accounts = Vec::new();
            for meta in &ix.accounts {
                let idx = account_keys
                    .iter()
                    .position(|k| *k == meta.pubkey)
                    .ok_or_else(|| format!("Account {} not found in account_keys", meta.pubkey))?
                    as u8;
                accounts.push(idx);
            }

            compiled_instructions.push(CompiledInstruction {
                program_id_index,
                accounts,
                data: ix.data.clone(),
            });
        }

        Ok(VersionedMessage {
            header,
            account_keys,
            recent_blockhash,
            instructions: compiled_instructions,
            address_table_lookups: Vec::new(),
        })
    }

    /// Serialize message into wire bytes (Bincode/Solana binary format).
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Version prefix: 1 byte 0x80 for v0 message if address_table_lookups not empty, or pure legacy if empty.
        if !self.address_table_lookups.is_empty() {
            buf.push(0x80); // Version 0
        }

        // Header
        buf.push(self.header.num_required_signatures);
        buf.push(self.header.num_readonly_signed_accounts);
        buf.push(self.header.num_readonly_unsigned_accounts);

        // Account keys (short-vec encoded length + 32 bytes per key)
        encode_short_vec(&mut buf, self.account_keys.len());
        for key in &self.account_keys {
            buf.extend_from_slice(&key.to_bytes());
        }

        // Recent blockhash
        buf.extend_from_slice(&self.recent_blockhash.to_bytes());

        // Instructions
        encode_short_vec(&mut buf, self.instructions.len());
        for ix in &self.instructions {
            buf.push(ix.program_id_index);
            encode_short_vec(&mut buf, ix.accounts.len());
            buf.extend_from_slice(&ix.accounts);
            encode_short_vec(&mut buf, ix.data.len());
            buf.extend_from_slice(&ix.data);
        }

        // Address table lookups (if v0)
        if !self.address_table_lookups.is_empty() {
            encode_short_vec(&mut buf, self.address_table_lookups.len());
            for lookup in &self.address_table_lookups {
                buf.extend_from_slice(&lookup.account_key.to_bytes());
                encode_short_vec(&mut buf, lookup.writable_indexes.len());
                buf.extend_from_slice(&lookup.writable_indexes);
                encode_short_vec(&mut buf, lookup.readonly_indexes.len());
                buf.extend_from_slice(&lookup.readonly_indexes);
            }
        }

        buf
    }
}

/// Encode compact u16 length (Solana short_vec).
fn encode_short_vec(buf: &mut Vec<u8>, mut len: usize) {
    loop {
        let mut elem = (len & 0x7f) as u8;
        len >>= 7;
        if len == 0 {
            buf.push(elem);
            break;
        } else {
            elem |= 0x80;
            buf.push(elem);
        }
    }
}

/// Unsigned Versioned Transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedTransaction {
    pub signatures: Vec<[u8; 64]>,
    pub message: VersionedMessage,
}

impl VersionedTransaction {
    pub fn new_unsigned(message: VersionedMessage) -> Self {
        let num_sigs = message.header.num_required_signatures as usize;
        let signatures = vec![[0u8; 64]; num_sigs];
        Self { signatures, message }
    }

    /// Serialize transaction to wire bytes.
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Signatures (short_vec)
        encode_short_vec(&mut buf, self.signatures.len());
        for sig in &self.signatures {
            buf.extend_from_slice(sig);
        }
        // Compiled message
        buf.extend_from_slice(&self.message.serialize());
        buf
    }

    /// Serialize transaction and encode as base64 string for wallet signing or Squads multisig.
    pub fn to_base64(&self) -> String {
        BASE64_STANDARD.encode(self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_compilation_and_base64() {
        let payer = Pubkey::system_program();
        let to = Pubkey::spl_token();
        let ix = Instruction::system_transfer(&payer, &to, 500_000);
        let blockhash = Pubkey::spl_token(); // dummy blockhash

        let message = VersionedMessage::compile(payer, &[ix], blockhash).unwrap();
        assert_eq!(message.header.num_required_signatures, 1);
        assert_eq!(message.account_keys[0], payer);

        let tx = VersionedTransaction::new_unsigned(message);
        assert_eq!(tx.signatures.len(), 1);
        let b64 = tx.to_base64();
        assert!(!b64.is_empty());
    }
}
