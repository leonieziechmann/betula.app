//! How keys and ids are written: base64url without padding on the wire (docs/phloem/protocol.md
//! §3), and the recovery code a person writes down: the profile key in base32 (Crockford: no I, L,
//! O, U; read in either case, O as 0, I and L as 1), in groups of four. The 256 bits of the key fill
//! 52 letters with 4 bits to spare, which carry a check: most mistyped codes are told at once,
//! before any request.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

/// The bytes of a profile key.
pub const KEY: usize = 32;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn unb64(text: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(text.trim()).ok()
}

/// A CRC-8 (polynomial 0x07) of the key; its low four bits are the code's check.
fn check(key: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for byte in key {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x07 } else { crc << 1 };
        }
    }
    crc & 0x0f
}

/// The recovery code of a key: 13 groups of four letters.
pub fn recovery_code(key: &[u8; KEY]) -> String {
    let mut bits: Vec<bool> = key.iter().flat_map(|byte| (0..8).rev().map(move |i| byte >> i & 1 == 1)).collect();
    let c = check(key);
    bits.extend((0..4).rev().map(|i| c >> i & 1 == 1));
    let letters: Vec<char> = bits
        .chunks(5)
        .map(|chunk| {
            let value = chunk.iter().fold(0usize, |v, bit| v << 1 | usize::from(*bit));
            CROCKFORD.get(value).map_or('0', |b| char::from(*b))
        })
        .collect();
    letters.chunks(4).map(|group| group.iter().collect::<String>()).collect::<Vec<_>>().join("-")
}

/// The key of a recovery code as a person types it: spaces and dashes anywhere, either case.
/// `None` where it is no code, or its check fails (mistyped).
pub fn key_of_code(code: &str) -> Option<[u8; KEY]> {
    let mut values: Vec<u8> = Vec::new();
    for c in code.chars().filter(|c| !c.is_whitespace() && *c != '-') {
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        let value = CROCKFORD.iter().position(|b| char::from(*b) == c)?;
        values.push(u8::try_from(value).ok()?);
    }
    if values.len() != 52 {
        return None;
    }
    let bits: Vec<bool> = values.iter().flat_map(|v| (0..5).rev().map(move |i| v >> i & 1 == 1)).collect();
    let mut key = [0u8; KEY];
    for (byte, chunk) in key.iter_mut().zip(bits.chunks(8)) {
        *byte = chunk.iter().fold(0u8, |v, bit| v << 1 | u8::from(*bit));
    }
    let c = bits.get(256..260)?.iter().fold(0u8, |v, bit| v << 1 | u8::from(*bit));
    (c == check(&key)).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_comes_back_as_its_key_and_a_typo_is_told() {
        let key: [u8; KEY] = core::array::from_fn(|i| (i as u8).wrapping_mul(37).wrapping_add(11));
        let code = recovery_code(&key);
        assert_eq!(code.len(), 52 + 12, "{code}");
        assert_eq!(code.split('-').count(), 13);
        assert_eq!(key_of_code(&code), Some(key));
        assert_eq!(key_of_code(&code.to_lowercase().replace('-', " ")), Some(key));
        assert_eq!(key_of_code(&code.replace('0', "o").replace('1', "l")), Some(key));
        let mut told = 0;
        for at in 0..code.len() {
            let mut typo: Vec<char> = code.chars().collect();
            if typo[at] == '-' {
                continue;
            }
            typo[at] = if typo[at] == 'X' { 'Y' } else { 'X' };
            if key_of_code(&typo.into_iter().collect::<String>()).is_none() {
                told += 1;
            }
        }
        assert!(told >= 40, "{told} of 52 typos told");
        assert_eq!(key_of_code("ABCD"), None);
        assert_eq!(key_of_code(&format!("{code}0")), None);
    }

    #[test]
    fn base64url_has_no_padding() {
        assert_eq!(b64(&[0xfb, 0xff]), "-_8");
        assert_eq!(unb64("-_8"), Some(vec![0xfb, 0xff]));
        assert_eq!(unb64("-_8="), None);
    }
}
