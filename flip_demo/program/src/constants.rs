use pinocchio::Address;

/// Program id for `flip-program-keypair.json` (pubkey `BPppJw92aJEF4PC1jdVWFGSc3SJCCH6BJveDpu786S7V`).
pub const ID: Address = Address::new_from_array([
   154, 109, 182, 97, 43, 39, 204, 209, 34, 198, 224, 61, 28, 185, 127, 84, 153, 147, 152, 236, 206, 158, 19, 170, 199, 74, 20, 157, 59, 174, 70, 116,
]);

/// Vault program id (must match `program/src/constants.rs` `ID`).
pub const VAULT_PROGRAM: Address = Address::new_from_array([
   0x8c, 0x4a, 0x75, 0xd5, 0xd6, 0x63, 0xca, 0x33, 0x5e, 0x25, 0xdb, 0xba, 0x06, 0x9f, 0x22, 0x80, 0xdc, 0x92, 0x04, 0xda, 0xbf, 0xc7, 0x8c, 0x86, 0xe9, 0x76, 0x8b, 0xae, 0xfe, 0xd3, 0x73, 0xcc,
]);

pub const CONFIG_SEED: &[u8] = b"config";

/// Devnet SPL USDC (`Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr`).
pub const USDC_MINT: Address = Address::new_from_array([
   233, 40, 57, 85, 9, 101, 255, 212, 214, 74, 202, 175, 70, 212, 93, 247, 49, 142, 91, 79, 87, 201, 12, 72, 125, 96, 98, 93, 130, 155, 131, 123,
]);

pub const CONFIG_DATA_LEN: usize = 1;
