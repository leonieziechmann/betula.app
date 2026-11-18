//! The standard security handler (ISO 32000-1 §7.6.3) with RC4, for a file opened without a
//! password: the BTU writes its Notenübersicht encrypted (RC4, 128 bits) with an empty user
//! password, so that any reader opens it, and an owner password that asks readers not to let it be
//! copied. Reading it is what the user password allows: the file's permissions (`/P`) grant the
//! extraction of its text „in support of accessibility … or for other purposes" (bit 10), which is
//! what reading one's own transcript into one's own plan is.
//!
//! MD5 (RFC 1321) and RC4 are here because the key is made with the one and the data read with the
//! other; neither protects anything in this use. AES (`/V 4` with AESV2, `/V 5`) is not read.

use super::syntax::{Dict, Object};

/// The padding of a password (Algorithm 2, step a).
const PAD: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C,
    0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

/// Why a file cannot be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It opens only with a password.
    Password,
    /// Its encryption is not RC4 of the standard handler.
    Unsupported,
}

/// The key of a file's RC4 encryption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    key: Vec<u8>,
}

impl Key {
    /// The key of the encryption dictionary `encrypt` with the file's first `/ID`, opened with the
    /// empty user password (Algorithms 2 and 6).
    pub fn open(encrypt: &Dict, id: &[u8]) -> Result<Key, Refusal> {
        if !encrypt.is(b"Filter", b"Standard") {
            return Err(Refusal::Unsupported);
        }
        let v = encrypt.get(b"V").and_then(Object::int).unwrap_or(0);
        let r = encrypt.get(b"R").and_then(Object::int).unwrap_or(2);
        let mut bits = encrypt.get(b"Length").and_then(Object::int).unwrap_or(40);
        match v {
            1 | 2 => {}
            // Crypt filters: RC4 (`/V2`) is read, AES is not.
            4 => {
                let filter = encrypt.get(b"StmF").and_then(Object::name).unwrap_or(b"Identity");
                if filter != b"Identity" {
                    let method = encrypt.get(b"CF").and_then(Object::dict).and_then(|cf| cf.get(filter)).and_then(Object::dict);
                    if !method.is_some_and(|method| method.is(b"CFM", b"V2")) {
                        return Err(Refusal::Unsupported);
                    }
                    if let Some(length) = method.and_then(|method| method.get(b"Length")).and_then(Object::int) {
                        // Bytes here (§7.6.5, Table 25), sometimes bits.
                        bits = if length <= 16 { length * 8 } else { length };
                    }
                }
            }
            _ => return Err(Refusal::Unsupported),
        }
        if !(2..=4).contains(&r) {
            return Err(Refusal::Unsupported);
        }
        let n = if r == 2 { 5 } else { usize::try_from(bits / 8).ok().filter(|n| (5..=16).contains(n)).ok_or(Refusal::Unsupported)? };
        let o = encrypt.get(b"O").and_then(Object::string).ok_or(Refusal::Unsupported)?;
        let u = encrypt.get(b"U").and_then(Object::string).ok_or(Refusal::Unsupported)?;
        let p = encrypt.get(b"P").and_then(Object::int).ok_or(Refusal::Unsupported)?;
        let metadata = !matches!(encrypt.get(b"EncryptMetadata"), Some(Object::Bool(false)));

        // Algorithm 2: the empty password, padded.
        let mut input = PAD.to_vec();
        input.extend_from_slice(o.get(..32).unwrap_or(o));
        input.extend_from_slice(&(p as i32 as u32).to_le_bytes());
        input.extend_from_slice(id);
        if r >= 4 && !metadata {
            input.extend_from_slice(&[0xff; 4]);
        }
        let mut hash = md5(&input);
        if r >= 3 {
            for _ in 0..50 {
                hash = md5(hash.get(..n).unwrap_or(&hash));
            }
        }
        let key = hash.get(..n).unwrap_or(&hash).to_vec();

        // Algorithm 6: the user password is the empty one where U is what it makes.
        let opens = if r == 2 {
            rc4(&key, &PAD) == u.get(..32).unwrap_or(u)
        } else {
            let mut check = PAD.to_vec();
            check.extend_from_slice(id);
            let mut made = rc4(&key, &md5(&check));
            for i in 1..=19u8 {
                let step: Vec<u8> = key.iter().map(|b| b ^ i).collect();
                made = rc4(&step, &made);
            }
            u.get(..16) == made.get(..16)
        };
        if opens {
            Ok(Key { key })
        } else {
            Err(Refusal::Password)
        }
    }

    /// The data of object `num` `gen` as written, read (Algorithm 1): RC4 with the key of the
    /// object.
    pub fn decrypt(&self, num: u32, gen: u16, data: &[u8]) -> Vec<u8> {
        let mut input = self.key.clone();
        input.extend_from_slice(num.to_le_bytes().get(..3).unwrap_or_default());
        input.extend_from_slice(&gen.to_le_bytes());
        let hash = md5(&input);
        let n = (self.key.len() + 5).min(16);
        rc4(hash.get(..n).unwrap_or(&hash), data)
    }
}

/// RC4: the key stream added to `data`.
pub fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut s: Vec<u8> = (0..=255u8).collect();
    if key.is_empty() {
        return data.to_vec();
    }
    let mut j = 0u8;
    for i in 0..256usize {
        let k = key.get(i % key.len()).copied().unwrap_or(0);
        let si = s.get(i).copied().unwrap_or(0);
        j = j.wrapping_add(si).wrapping_add(k);
        s.swap(i, usize::from(j));
    }
    let (mut i, mut j) = (0u8, 0u8);
    data.iter()
        .map(|b| {
            i = i.wrapping_add(1);
            let si = s.get(usize::from(i)).copied().unwrap_or(0);
            j = j.wrapping_add(si);
            s.swap(usize::from(i), usize::from(j));
            let sj = s.get(usize::from(i)).copied().unwrap_or(0);
            let t = si.wrapping_add(sj);
            b ^ s.get(usize::from(t)).copied().unwrap_or(0)
        })
        .collect()
}

/// MD5 of `data` (RFC 1321).
pub fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4,
        11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // ⌊|sin(i + 1)| · 2³²⌋, as RFC 1321 lists them.
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64).wrapping_mul(8)).to_le_bytes());
    let (mut a0, mut b0, mut c0, mut d0): (u32, u32, u32, u32) = (0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476);
    for chunk in message.chunks_exact(64) {
        let m: Vec<u32> = chunk.chunks_exact(4).map(|w| u32::from_le_bytes([w.first().copied().unwrap_or(0), w.get(1).copied().unwrap_or(0), w.get(2).copied().unwrap_or(0), w.get(3).copied().unwrap_or(0)])).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64usize {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(K.get(i).copied().unwrap_or(0)).wrapping_add(m.get(g).copied().unwrap_or(0));
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S.get(i).copied().unwrap_or(0)));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (slot, byte) in out.iter_mut().zip(a0.to_le_bytes().into_iter().chain(b0.to_le_bytes()).chain(c0.to_le_bytes()).chain(d0.to_le_bytes())) {
        *slot = byte;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn md5_of_rfc_1321() {
        assert_eq!(hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hex(&md5(b"message digest")), "f96b697d7cb7938d525a2f31aaf161d0");
        assert_eq!(hex(&md5(b"12345678901234567890123456789012345678901234567890123456789012345678901234567890")), "57edf4a22be3c955ac49da2e2107b67a");
    }

    #[test]
    fn rc4_of_its_test_vectors() {
        assert_eq!(hex(&rc4(b"Key", b"Plaintext")), "bbf316e8d940af0ad3");
        assert_eq!(hex(&rc4(b"Wiki", b"pedia")), "1021bf0420");
        // Twice is the data again.
        assert_eq!(rc4(b"Secret", &rc4(b"Secret", b"Attack at dawn")), b"Attack at dawn");
    }
}
