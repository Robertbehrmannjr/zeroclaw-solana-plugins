use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

/// 32-byte Solana Public Key / Program Address.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Pubkey(pub [u8; 32]);

impl Pubkey {
    pub const SYSTEM_PROGRAM_ID: Pubkey = Pubkey([0u8; 32]);
    pub const SPL_TOKEN_PROGRAM_ID: Pubkey = Pubkey([
        6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180,
        133, 237, 95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
    ]);
    pub const SPL_ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = Pubkey([
        140, 151, 37, 143, 78, 36, 137, 241, 187, 61, 16, 41, 20, 142, 13, 131, 11, 90, 19,
        153, 218, 255, 16, 132, 4, 142, 123, 216, 219, 233, 248, 89,
    ]);
    pub const TOKEN_2022_PROGRAM_ID: Pubkey = Pubkey([
        6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180,
        133, 237, 95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 170,
    ]);
    pub const MEMO_V2_PROGRAM_ID: Pubkey = Pubkey([
        5, 74, 83, 90, 153, 41, 33, 6, 77, 36, 232, 113, 96, 218, 56, 124, 124, 53, 181, 21,
        8, 13, 86, 13, 23, 238, 25, 34, 11, 238, 2, 85,
    ]);
    pub const SYSVAR_RECENT_BLOCKHASH_PUBKEY: Pubkey = Pubkey([
        6, 167, 213, 23, 25, 44, 86, 142, 224, 138, 132, 95, 115, 210, 151, 136, 207, 3, 92,
        49, 69, 178, 26, 183, 68, 83, 211, 0, 0, 0, 0, 0,
    ]);
    pub const SYSVAR_RENT_PUBKEY: Pubkey = Pubkey([
        6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184,
        163, 155, 75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
    ]);

    pub fn new_from_array(bytes: [u8; 32]) -> Self {
        Pubkey(bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 32 {
            return Err(format!("Invalid pubkey byte length: expected <= 32, got {}", bytes.len()));
        }
        let mut arr = [0u8; 32];
        let offset = 32 - bytes.len();
        arr[offset..].copy_from_slice(bytes);
        Ok(Pubkey(arr))
    }

    /// Check if this pubkey matches known token programs (SPL Token or Token-2022).
    pub fn is_token_program(&self) -> bool {
        *self == Self::SPL_TOKEN_PROGRAM_ID || *self == Self::TOKEN_2022_PROGRAM_ID
    }

    pub fn system_program() -> Self {
        Self::SYSTEM_PROGRAM_ID
    }

    pub fn spl_token() -> Self {
        Self::SPL_TOKEN_PROGRAM_ID
    }

    pub fn associated_token_program() -> Self {
        Self::SPL_ASSOCIATED_TOKEN_PROGRAM_ID
    }

    pub fn token_2022() -> Self {
        Self::TOKEN_2022_PROGRAM_ID
    }

    pub fn memo_v2() -> Self {
        Self::MEMO_V2_PROGRAM_ID
    }

    /// Find Program Address (PDA) with bump seed.
    pub fn find_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Option<(Pubkey, u8)> {
        for bump_seed in (0..=255u8).rev() {
            let mut all_seeds = Vec::new();
            for s in seeds {
                all_seeds.push(*s);
            }
            let bump_slice = &[bump_seed];
            all_seeds.push(bump_slice);

            if let Some(pubkey) = Self::create_program_address(&all_seeds, program_id) {
                return Some((pubkey, bump_seed));
            }
        }
        None
    }

    /// Create Program Address if it does not lie on the ed25519 curve.
    pub fn create_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Option<Pubkey> {
        let mut hasher = Sha256::new();
        for seed in seeds {
            hasher.update(seed);
        }
        hasher.update(program_id.to_bytes());
        hasher.update(b"ProgramDerivedAddress");
        let hash = hasher.finalize();

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hash[..32]);
        Some(Pubkey(bytes))
    }

    /// Compute Associated Token Account address for a wallet and mint.
    pub fn get_associated_token_address(wallet: &Pubkey, mint: &Pubkey, token_program_id: &Pubkey) -> Pubkey {
        Self::find_program_address(
            &[&wallet.0, &token_program_id.0, &mint.0],
            &Self::SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        )
        .map(|(pda, _)| pda)
        .unwrap_or_default()
    }
}

impl FromStr for Pubkey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = bs58::decode(s)
            .into_vec()
            .map_err(|e| format!("bs58 decode error for pubkey '{}': {}", s, e))?;
        Self::from_bytes(&bytes)
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", bs58::encode(&self.0).into_string())
    }
}

impl fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pubkey({})", self)
    }
}

impl Hash for Pubkey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Serialize for Pubkey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Pubkey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Pubkey::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pubkey_constants_encode_correctly() {
        let spl = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();
        assert_eq!(spl.to_string(), "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

        let t22 = Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXBhDes").unwrap();
        assert_eq!(Pubkey::from_bytes(&t22.to_bytes()).unwrap(), t22);
    }

    #[test]
    fn test_ata_derivation() {
        let wallet = Pubkey::from_str("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM").unwrap();
        let mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(); // USDC
        let ata = Pubkey::get_associated_token_address(&wallet, &mint, &Pubkey::spl_token());
        assert_ne!(ata, Pubkey::default());
        assert_eq!(ata.to_bytes().len(), 32);
    }
}
