//! Aster OS identity core: username rules, password hashing and the account DB.
//!
//! Password hashing is PBKDF2-HMAC-SHA256 implemented directly from FIPS 180-4,
//! RFC 2104 and RFC 8018, with the published test vectors in `tests`. No
//! plaintext password is ever stored or serialized.
//!
//! 0.8.0 is authentication and UX only. Kernel-enforced file permissions and
//! multi-user isolation follow in 0.8.1 (AFS v2).

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const MIN_USERNAME: usize = 3;
pub const MAX_USERNAME: usize = 32;
pub const MIN_UID: u32 = 1000;
pub const PBKDF2_ITERATIONS: u32 = 100_000;
pub const SALT_LEN: usize = 16;
pub const HASH_LEN: usize = 32;
pub const DB_VERSION: u32 = 1;

const RESERVED: [&str; 18] = [
    "root", "admin", "administrator", "system", "daemon", "bin", "sys", "user",
    "aster", "configd", "desk", "nobody", "guest", "login", "lock", "setup",
    "kernel", "operator",
];

/// Lowercase ASCII letters, digits, `.`, `_` and `-`; 3..=32 characters.
pub fn validate_username(name: &str) -> Result<(), &'static str> {
    if name.len() < MIN_USERNAME || name.len() > MAX_USERNAME {
        return Err("Username must be 3-32 characters");
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err("Use letters, digits, dot, underscore or hyphen");
    }
    if name.as_bytes()[0] == b'.' || name.as_bytes()[0] == b'-' {
        return Err("Username cannot start with a dot or hyphen");
    }
    let lower = name.to_ascii_lowercase();
    if RESERVED.contains(&lower.as_str()) {
        return Err("This username is reserved");
    }
    Ok(())
}

/// Full names are display-only but must not contain control characters or tabs.
pub fn validate_full_name(name: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() || name.len() > 64 {
        return Err("Enter a display name (max 64 characters)");
    }
    if name.chars().any(|ch| ch.is_control()) {
        return Err("The display name contains control characters");
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters");
    }
    if password.len() > 128 {
        return Err("Password must be at most 128 characters");
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0F) as usize] as char);
    }
    output
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let mut output = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for pair in bytes.chunks(2) {
        let high = (pair[0] as char).to_digit(16)? as u8;
        let low = (pair[1] as char).to_digit(16)? as u8;
        output.push((high << 4) | low);
    }
    Some(output)
}

// ---------------------------------------------------------------------------
// SHA-256 / HMAC / PBKDF2
// ---------------------------------------------------------------------------

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
    0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn compress(state: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];
    for index in 0..16 {
        w[index] = u32::from_be_bytes([
            block[index * 4],
            block[index * 4 + 1],
            block[index * 4 + 2],
            block[index * 4 + 3],
        ]);
    }
    for index in 16..64 {
        let s0 = w[index - 15].rotate_right(7)
            ^ w[index - 15].rotate_right(18)
            ^ (w[index - 15] >> 3);
        let s1 = w[index - 2].rotate_right(17)
            ^ w[index - 2].rotate_right(19)
            ^ (w[index - 2] >> 10);
        w[index] = w[index - 16]
            .wrapping_add(s0)
            .wrapping_add(w[index - 7])
            .wrapping_add(s1);
    }
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];
    for index in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[index])
            .wrapping_add(w[index]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state = [
        0x6a09e667u32, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let mut offset = 0;
    while offset + 64 <= data.len() {
        compress(&mut state, &data[offset..offset + 64]);
        offset += 64;
    }
    let mut tail = [0u8; 128];
    let remaining = data.len() - offset;
    tail[..remaining].copy_from_slice(&data[offset..]);
    tail[remaining] = 0x80;
    let total = if remaining < 56 { 64 } else { 128 };
    let bit_len = (data.len() as u64).wrapping_mul(8);
    tail[total - 8..total].copy_from_slice(&bit_len.to_be_bytes());
    for block_start in (0..total).step_by(64) {
        compress(&mut state, &tail[block_start..block_start + 64]);
    }
    let mut digest = [0u8; 32];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&sha256(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = [0u8; 64];
    let mut outer = [0u8; 64];
    for index in 0..64 {
        inner[index] = block[index] ^ 0x36;
        outer[index] = block[index] ^ 0x5c;
    }
    let mut inner_data = Vec::with_capacity(64 + data.len());
    inner_data.extend_from_slice(&inner);
    inner_data.extend_from_slice(data);
    let inner_digest = sha256(&inner_data);
    let mut outer_data = Vec::with_capacity(96);
    outer_data.extend_from_slice(&outer);
    outer_data.extend_from_slice(&inner_digest);
    sha256(&outer_data)
}

/// PBKDF2-HMAC-SHA256, one 32-byte block (dkLen = 32).
pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let iterations = iterations.max(1);
    let mut block = Vec::with_capacity(salt.len() + 4);
    block.extend_from_slice(salt);
    block.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha256(password, &block);
    let mut output = u;
    for _ in 1..iterations {
        u = hmac_sha256(password, &u);
        for index in 0..32 {
            output[index] ^= u[index];
        }
    }
    output
}

pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

// ---------------------------------------------------------------------------
// Account database
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Account {
    pub username: String,
    pub display_name: String,
    pub uid: u32,
    pub gid: u32,
    pub admin: bool,
    pub home: String,
    pub salt: [u8; SALT_LEN],
    pub hash: [u8; HASH_LEN],
    pub iterations: u32,
    pub created: u64,
    pub last_login: u64,
}

impl Account {
    pub fn new(
        username: &str,
        display_name: &str,
        uid: u32,
        admin: bool,
        home: String,
        password: &str,
        salt: [u8; SALT_LEN],
        iterations: u32,
        created: u64,
    ) -> Result<Account, &'static str> {
        validate_username(username)?;
        validate_full_name(display_name)?;
        validate_password(password)?;
        if uid < MIN_UID {
            return Err("UID is reserved");
        }
        let hash = pbkdf2_sha256(password.as_bytes(), &salt, iterations);
        Ok(Account {
            username: username.to_string(),
            display_name: display_name.to_string(),
            uid,
            gid: uid,
            admin,
            home,
            salt,
            hash,
            iterations,
            created,
            last_login: 0,
        })
    }

    pub fn verify(&self, password: &str) -> bool {
        let candidate = pbkdf2_sha256(password.as_bytes(), &self.salt, self.iterations);
        constant_time_eq(&candidate, &self.hash)
    }

    pub fn change_password(&mut self, password: &str, salt: [u8; SALT_LEN], iterations: u32) -> Result<(), &'static str> {
        validate_password(password)?;
        self.salt = salt;
        self.iterations = iterations;
        self.hash = pbkdf2_sha256(password.as_bytes(), &salt, iterations);
        Ok(())
    }
}

#[derive(Default)]
pub struct AccountDb {
    pub accounts: Vec<Account>,
}

impl AccountDb {
    pub fn new() -> Self {
        Self { accounts: Vec::new() }
    }

    pub fn find(&self, username: &str) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.username.eq_ignore_ascii_case(username))
    }

    pub fn find_mut(&mut self, username: &str) -> Option<&mut Account> {
        self.accounts
            .iter_mut()
            .find(|account| account.username.eq_ignore_ascii_case(username))
    }

    pub fn add(&mut self, account: Account) -> Result<(), &'static str> {
        if self.find(&account.username).is_some() {
            return Err("Username already exists");
        }
        if self.accounts.iter().any(|existing| existing.uid == account.uid) {
            return Err("UID already exists");
        }
        self.accounts.push(account);
        self.accounts
            .sort_by(|left, right| left.username.to_ascii_lowercase().cmp(&right.username.to_ascii_lowercase()));
        Ok(())
    }

    pub fn remove(&mut self, username: &str) -> bool {
        if let Some(index) = self
            .accounts
            .iter()
            .position(|account| account.username.eq_ignore_ascii_case(username))
        {
            self.accounts.remove(index);
            true
        } else {
            false
        }
    }

    pub fn next_uid(&self) -> u32 {
        self.accounts
            .iter()
            .map(|account| account.uid)
            .max()
            .unwrap_or(MIN_UID - 1)
            .saturating_add(1)
            .max(MIN_UID)
    }

    pub fn serialize(&self) -> String {
        let mut output = format!("ASTER-ACCOUNTS {}\n", DB_VERSION);
        for account in &self.accounts {
            output.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                account.username,
                account.display_name,
                account.uid,
                account.gid,
                account.admin as u8,
                account.home,
                hex_encode(&account.salt),
                hex_encode(&account.hash),
                account.iterations,
                account.created,
                account.last_login
            ));
        }
        output
    }

    pub fn parse(data: &str) -> Result<AccountDb, &'static str> {
        let mut lines = data.lines();
        let header = lines.next().ok_or("Empty account database")?;
        let mut header_parts = header.split_whitespace();
        if header_parts.next() != Some("ASTER-ACCOUNTS") {
            return Err("Invalid account database header");
        }
        let version: u32 = header_parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or("Invalid account database version")?;
        if version != DB_VERSION {
            return Err("Unsupported account database version");
        }
        let mut db = AccountDb::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() != 11 {
                return Err("Malformed account record");
            }
            let uid: u32 = fields[2].parse().map_err(|_| "Bad UID")?;
            let gid: u32 = fields[3].parse().map_err(|_| "Bad GID")?;
            let admin = fields[4] == "1";
            let salt_bytes = hex_decode(fields[6]).ok_or("Bad salt")?;
            let hash_bytes = hex_decode(fields[7]).ok_or("Bad hash")?;
            if salt_bytes.len() != SALT_LEN || hash_bytes.len() != HASH_LEN {
                return Err("Bad salt/hash length");
            }
            let mut salt = [0u8; SALT_LEN];
            salt.copy_from_slice(&salt_bytes);
            let mut hash = [0u8; HASH_LEN];
            hash.copy_from_slice(&hash_bytes);
            let account = Account {
                username: fields[0].to_string(),
                display_name: fields[1].to_string(),
                uid,
                gid,
                admin,
                home: fields[5].to_string(),
                salt,
                hash,
                iterations: fields[8].parse().map_err(|_| "Bad KDF iterations")?,
                created: fields[9].parse().map_err(|_| "Bad created time")?,
                last_login: fields[10].parse().map_err(|_| "Bad login time")?,
            };
            validate_username(&account.username)?;
            db.add(account)?;
        }
        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        hex_encode(bytes)
    }

    #[test]
    fn sha256_vectors() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn hmac_rfc4231_vectors() {
        assert_eq!(
            hex(&hmac_sha256(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn pbkdf2_sha256_vectors() {
        assert_eq!(
            hex(&pbkdf2_sha256(b"password", b"salt", 1)),
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
        assert_eq!(
            hex(&pbkdf2_sha256(b"password", b"salt", 2)),
            "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
        );
        assert_eq!(
            hex(&pbkdf2_sha256(b"password", b"salt", 4096)),
            "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
        );
    }

    #[test]
    fn username_rules() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("a.b-c_1").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("root").is_err());
        assert!(validate_username("Admin").is_err());
        assert!(validate_username(".alice").is_err());
        assert!(validate_username("alice space").is_err());
        assert!(validate_username(&"a".repeat(33)).is_err());
    }

    #[test]
    fn account_roundtrip_and_verify() {
        let salt = [0x42u8; SALT_LEN];
        let alice = Account::new(
            "alice",
            "Alice Example",
            1000,
            true,
            "/home/U1000".to_string(),
            "correct horse battery",
            salt,
            10,
            1234,
        )
        .unwrap();
        let bob = Account::new(
            "bob",
            "Bob Example",
            1001,
            false,
            "/home/U1001".to_string(),
            "another password",
            [0x24u8; SALT_LEN],
            10,
            1235,
        )
        .unwrap();
        let mut db = AccountDb::new();
        db.add(alice).unwrap();
        db.add(bob).unwrap();
        assert_eq!(db.next_uid(), 1002);
        let duplicate = Account::new(
            "ALICE",
            "Duplicate",
            1002,
            false,
            "/home/U1002".to_string(),
            "password123",
            salt,
            10,
            1,
        )
        .unwrap();
        assert_eq!(db.add(duplicate).unwrap_err(), "Username already exists");

        let encoded = db.serialize();
        assert!(!encoded.contains("correct horse battery"));
        let parsed = AccountDb::parse(&encoded).unwrap();
        assert!(parsed.find("ALICE").unwrap().verify("correct horse battery"));
        assert!(!parsed.find("alice").unwrap().verify("wrong password"));
        assert!(parsed.find("bob").unwrap().verify("another password"));
        assert!(parsed.find("Alice").unwrap().admin);
        assert_eq!(parsed.find("alice").unwrap().uid, 1000);
        assert_eq!(parsed.find("alice").unwrap().home, "/home/U1000");
    }

    #[test]
    fn rejects_malformed_db() {
        assert!(AccountDb::parse("").is_err());
        assert!(AccountDb::parse("ASTER-ACCOUNTS 2\n").is_err());
        assert!(AccountDb::parse("ASTER-ACCOUNTS 1\nbad\n").is_err());
    }
}
