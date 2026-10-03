//! A number in the characters an address carries as they are, and the two characters that check it.
//!
//! The payload is the number in base 66, its lowest digit first and without zeros at its high end,
//! so every number has one spelling. The check value is 4091 at most, a prime: two characters could
//! say 66 · 62 = 4092 values, the second one taken from the letters and digits only (`ALPHABET`).
//!
//! What the check characters catch for certain, wherever it happens in a code, their own place
//! included: a character typed wrong, two characters swapped that stand next to each other or one
//! apart, and two neighbours typed wrong alike (`aa` for `bb`). The check value is the payload read
//! as a number in base 66, each digit counted one higher, modulo the prime. Each of these mistakes
//! moves it (or what the check characters state, or the two apart) by a difference of two digits
//! times a factor: 66^i, 65 · 66^i, (66² - 1) · 66^i or 67 · 66^i within the payload; 63, 128, 2
//! or 61 across its end. Neither is a multiple of 4091, a prime, so the sum never is. Anything else
//! passes with a chance of 1 in 4091: a character dropped or doubled, a code cut short or run into
//! the text after it, a code of another kind.

use crate::bits::{significant, Bits, ZERO_TAIL};
use crate::{Error, MAX_LEN};

/// The characters of a code: the unreserved characters of RFC 3986, as the digits 0 to 65 in this
/// order. The first 62 are the letters and digits: the last character of a code is always one of
/// them, since it is what a chat or a mail program might otherwise take for the end of a sentence
/// (`…#m=abc.`) and leave out of the link it makes.
pub const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// The check value is taken modulo this prime.
const PRIME: u32 = 4091;
/// How many values the last character can say: the letters and digits.
const LAST: u32 = 62;
/// Five digits at once: 66^5 < 2^32, so a word and what is left of the division fit into 64 bits.
const CHUNK: u64 = 66 * 66 * 66 * 66 * 66;

fn symbol(digit: u8) -> char {
    match digit {
        0..=25 => char::from(b'A' + digit),
        26..=51 => char::from(b'a' + (digit - 26)),
        52..=61 => char::from(b'0' + (digit - 52)),
        62 => '-',
        63 => '.',
        64 => '_',
        _ => '~',
    }
}

fn digit(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'-' => Some(62),
        b'.' => Some(63),
        b'_' => Some(64),
        b'~' => Some(65),
        _ => None,
    }
}

/// The digits of a number in base 66, the lowest first; none for zero.
fn to_digits(words: &[u32]) -> Vec<u8> {
    let mut number: Vec<u32> = words.to_vec();
    let mut digits: Vec<u8> = Vec::new();
    loop {
        while number.last() == Some(&0) {
            number.pop();
        }
        if number.is_empty() {
            break;
        }
        let mut rest = 0u64;
        for word in number.iter_mut().rev() {
            let value = (rest << 32) | u64::from(*word);
            *word = (value / CHUNK) as u32;
            rest = value % CHUNK;
        }
        for _ in 0..5 {
            digits.push((rest % 66) as u8);
            rest /= 66;
        }
    }
    while digits.last() == Some(&0) {
        digits.pop();
    }
    digits
}

/// The number that digits in base 66 spell, the lowest digit first.
fn from_digits(digits: &[u8]) -> Vec<u32> {
    let mut number: Vec<u32> = Vec::new();
    for chunk in digits.chunks(5).rev() {
        let mut carry = chunk.iter().rev().fold(0u64, |value, digit| value * 66 + u64::from(*digit));
        for word in &mut number {
            let value = u64::from(*word) * CHUNK + carry;
            *word = value as u32;
            carry = value >> 32;
        }
        if carry > 0 {
            number.push(carry as u32);
        }
    }
    number
}

/// The check value of a payload for a kind of code: the bytes of the kind, then the digits, each
/// counted one higher, folded as a number in base 66 modulo the prime.
fn check(kind: &str, digits: &[u8]) -> u32 {
    kind.bytes().chain(digits.iter().copied()).fold(0, |value, digit| (value * 66 + u32::from(digit) + 1) % PRIME)
}

/// The code for these bits.
pub(crate) fn encode(kind: &str, bits: &Bits) -> Result<String, Error> {
    if bits.len() - significant(bits.words()) > ZERO_TAIL {
        return Err(Error::Unsupported("a value that ends in more than 65536 zero bits"));
    }
    let digits = to_digits(bits.words());
    if digits.len() + 2 > MAX_LEN {
        return Err(Error::TooLong);
    }
    let check = check(kind, &digits);
    let mut code: String = digits.iter().map(|digit| symbol(*digit)).collect();
    code.push(symbol((check / LAST) as u8));
    code.push(symbol((check % LAST) as u8));
    Ok(code)
}

/// The bits a code carries, as a number, if it is a code of this kind; and where they end.
pub(crate) fn decode(kind: &str, code: &str) -> Result<(Vec<u32>, usize), Error> {
    if code.len() > MAX_LEN {
        return Err(Error::TooLong);
    }
    let digits = code.bytes().enumerate().map(|(at, c)| digit(c).ok_or(Error::Character(at))).collect::<Result<Vec<u8>, Error>>()?;
    let (&last, rest) = digits.split_last().ok_or(Error::Check)?;
    let (&first, payload) = rest.split_last().ok_or(Error::Check)?;
    let stated = u32::from(first) * LAST + u32::from(last);
    if u32::from(last) >= LAST || stated >= PRIME || stated != check(kind, payload) {
        return Err(Error::Check);
    }
    // A number is written without zeros at its high end: this one has two spellings.
    if payload.last() == Some(&0) {
        return Err(Error::Malformed);
    }
    let number = from_digits(payload);
    let end = significant(&number);
    Ok((number, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A number of `words` words, all of them random.
    fn random(state: &mut u64, words: usize) -> Vec<u32> {
        (0..words).map(|_| crate::tests::next(state) as u32).collect()
    }

    #[test]
    fn the_alphabet_is_what_an_address_carries_as_it_is() {
        let symbols: Vec<char> = (0..66).map(symbol).collect();
        assert_eq!(symbols.iter().collect::<String>(), ALPHABET);
        // RFC 3986, 2.3: ALPHA / DIGIT / "-" / "." / "_" / "~", each once.
        let mut sorted = symbols.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 66);
        assert!(symbols.iter().all(|c| c.is_ascii_alphanumeric() || "-._~".contains(*c)));
        for (at, c) in ALPHABET.bytes().enumerate() {
            assert_eq!(digit(c), Some(at as u8));
        }
        assert_eq!((0..=255u8).filter(|c| digit(*c).is_some()).count(), 66);
        assert!(ALPHABET.chars().take(LAST as usize).all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn numbers_and_their_digits() {
        assert_eq!(to_digits(&[]), Vec::<u8>::new());
        assert_eq!(to_digits(&[0, 0]), Vec::<u8>::new());
        assert_eq!(to_digits(&[65]), [65]);
        assert_eq!(to_digits(&[66]), [0, 1]);
        assert_eq!(to_digits(&[4355]), [65, 65]);
        assert_eq!(from_digits(&to_digits(&[0, 1])), [0, 1]);
        let mut state = 7;
        for words in 0..40 {
            for _ in 0..20 {
                let mut number = random(&mut state, words);
                let digits = to_digits(&number);
                assert!(digits.iter().all(|digit| *digit < 66) && digits.last() != Some(&0));
                while number.last() == Some(&0) {
                    number.pop();
                }
                assert_eq!(from_digits(&digits), number);
            }
        }
        // As many digits as the number needs: 66^n - 1 takes n, 66^n takes n + 1.
        for n in 1..60 {
            let power = from_digits(&[vec![0u8; n], vec![1]].concat());
            assert_eq!(to_digits(&power).len(), n + 1);
            let below = from_digits(&vec![65u8; n]);
            assert_eq!(to_digits(&below).len(), n);
        }
    }

    #[test]
    fn the_check_value_is_taken_modulo_a_prime_with_66_as_a_primitive_root() {
        assert!((2..PRIME).take_while(|d| d * d <= PRIME).all(|d| !PRIME.is_multiple_of(d)));
        assert_eq!(PRIME, LAST * 66 - 1);
        // 66 has order 4090 = 2 · 5 · 409 modulo 4091.
        let power = |exponent: u32| (0..exponent).fold(1u32, |value, _| value * 66 % PRIME);
        assert!([2, 5, 409].iter().all(|factor| power((PRIME - 1) / factor) != 1));
        assert_eq!(power(PRIME - 1), 1);
    }

    /// Every code with one character typed wrong, two swapped that stand next to each other or one
    /// apart, or two neighbours alike typed wrong alike.
    fn typos(code: &str) -> Vec<String> {
        let bytes = code.as_bytes();
        let mut typos = Vec::new();
        for at in 0..bytes.len() {
            for c in ALPHABET.bytes().filter(|c| *c != bytes[at]) {
                let mut typo = bytes.to_vec();
                typo[at] = c;
                typos.push(String::from_utf8(typo).unwrap());
            }
            if at + 1 < bytes.len() && bytes[at] != bytes[at + 1] {
                let mut typo = bytes.to_vec();
                typo.swap(at, at + 1);
                typos.push(String::from_utf8(typo).unwrap());
            }
            if at + 2 < bytes.len() && bytes[at] != bytes[at + 2] {
                let mut typo = bytes.to_vec();
                typo.swap(at, at + 2);
                typos.push(String::from_utf8(typo).unwrap());
            }
            if at + 1 < bytes.len() && bytes[at] == bytes[at + 1] {
                for c in ALPHABET.bytes().filter(|c| *c != bytes[at]) {
                    let mut typo = bytes.to_vec();
                    typo[at] = c;
                    typo[at + 1] = c;
                    typos.push(String::from_utf8(typo).unwrap());
                }
            }
        }
        typos
    }

    #[test]
    fn a_character_typed_wrong_or_two_swapped_never_passes() {
        let mut state = 11;
        for words in [0, 0, 1, 1, 2, 5, 17] {
            let mut bits = Bits::default();
            for _ in 0..words * 32 {
                bits.push(crate::tests::next(&mut state) & 1 == 1);
            }
            bits.push(true);
            for kind in ["", "bookmarks", "plan"] {
                let code = encode(kind, &bits).unwrap();
                assert!(decode(kind, &code).is_ok());
                assert!(code.chars().last().is_some_and(|c| c.is_ascii_alphanumeric()), "{code}");
                for typo in typos(&code) {
                    assert!(decode(kind, &typo).is_err(), "{typo} passes for {code}");
                }
            }
        }
    }

    #[test]
    fn a_code_is_checked_before_it_is_read() {
        let mut bits = Bits::default();
        bits.push_bits(0xdead_beef, 32);
        let code = encode("bookmarks", &bits).unwrap();
        assert_eq!(decode("bookmarks", &code), Ok((vec![0xdead_beef], 32)));
        assert_eq!(decode("plan", &code), Err(Error::Check), "a code of another kind");
        assert_eq!(decode("bookmarks", &code[..code.len() - 1]), Err(Error::Check), "cut short");
        assert_eq!(decode("bookmarks", ""), Err(Error::Check));
        assert_eq!(decode("bookmarks", "A"), Err(Error::Check));
        assert_eq!(decode("bookmarks", &format!("{code} ")), Err(Error::Character(code.len())));
        assert_eq!(decode("bookmarks", &format!("{code}ä")), Err(Error::Character(code.len())));
        // Zero is the empty payload and its check characters.
        let zero = encode("bookmarks", &Bits::default()).unwrap();
        assert_eq!(zero.len(), 2);
        assert_eq!(decode("bookmarks", &zero), Ok((Vec::new(), 0)));
        // A zero at the high end is a second spelling of the same number.
        let payload = &code[..code.len() - 2];
        let padded: Vec<u8> = payload.bytes().map(|c| digit(c).unwrap()).chain([0]).collect();
        let check = check("bookmarks", &padded);
        let spelled = format!("{payload}A{}{}", symbol((check / LAST) as u8), symbol((check % LAST) as u8));
        assert_eq!(decode("bookmarks", &spelled), Err(Error::Malformed));
    }

    #[test]
    fn a_code_has_a_limit() {
        assert_eq!(decode("", &"A".repeat(MAX_LEN + 1)), Err(Error::TooLong));
        let mut ones = Bits::default();
        ones.push_bits(u64::MAX, 64);
        let mut long = Bits::default();
        while long.len() < MAX_LEN * 7 {
            long.append(&ones);
        }
        assert_eq!(encode("", &long), Err(Error::TooLong));
        // Just below it, there and back: as many words as the payload of the longest code holds.
        let words = ((MAX_LEN - 2) as f64 * 66f64.log2() / 32.0) as usize;
        let mut state = 5;
        let mut number = random(&mut state, words);
        if let Some(top) = number.last_mut() {
            *top |= 1 << 31;
        }
        let mut bits = Bits::default();
        for word in &number {
            bits.push_bits(u64::from(*word), 32);
        }
        let code = encode("", &bits).unwrap();
        assert!(code.len() <= MAX_LEN && code.len() > MAX_LEN - 5, "{}", code.len());
        assert_eq!(decode("", &code), Ok((number, words * 32)));
    }
}
