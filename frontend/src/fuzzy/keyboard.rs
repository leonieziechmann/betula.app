//! Keyboard proximity mapping for typo tolerance and adjacent key detection (QWERTZ/QWERTY).

/// Returns an approximate (row, col) coordinate for a character on a standard keyboard layout.
fn key_coords(c: char) -> Option<(f64, f64)> {
    let lower = c.to_ascii_lowercase();
    match lower {
        // Row 0 (Number row)
        '1' => Some((0.0, 1.0)),
        '2' => Some((0.0, 2.0)),
        '3' => Some((0.0, 3.0)),
        '4' => Some((0.0, 4.0)),
        '5' => Some((0.0, 5.0)),
        '6' => Some((0.0, 6.0)),
        '7' => Some((0.0, 7.0)),
        '8' => Some((0.0, 8.0)),
        '9' => Some((0.0, 9.0)),
        '0' => Some((0.0, 10.0)),
        'ß' => Some((0.0, 11.0)),

        // Row 1 (QWERTZ / QWERTY top letter row)
        'q' => Some((1.0, 1.5)),
        'w' => Some((1.0, 2.5)),
        'e' => Some((1.0, 3.5)),
        'r' => Some((1.0, 4.5)),
        't' => Some((1.0, 5.5)),
        'z' => Some((1.0, 6.5)), // QWERTZ z
        'u' => Some((1.0, 7.5)),
        'i' => Some((1.0, 8.5)),
        'o' => Some((1.0, 9.5)),
        'p' => Some((1.0, 10.5)),
        'ü' => Some((1.0, 11.5)),

        // Row 2 (Home row)
        'a' => Some((2.0, 1.8)),
        's' => Some((2.0, 2.8)),
        'd' => Some((2.0, 3.8)),
        'f' => Some((2.0, 4.8)),
        'g' => Some((2.0, 5.8)),
        'h' => Some((2.0, 6.8)),
        'j' => Some((2.0, 7.8)),
        'k' => Some((2.0, 8.8)),
        'l' => Some((2.0, 9.8)),
        'ö' => Some((2.0, 10.8)),
        'ä' => Some((2.0, 11.8)),

        // Row 3 (Bottom letter row)
        'y' => Some((3.0, 2.2)), // QWERTZ y
        'x' => Some((3.0, 3.2)),
        'c' => Some((3.0, 4.2)),
        'v' => Some((3.0, 5.2)),
        'b' => Some((3.0, 6.2)),
        'n' => Some((3.0, 7.2)),
        'm' => Some((3.0, 8.2)),
        ',' => Some((3.0, 9.2)),
        '.' => Some((3.0, 10.2)),
        '-' => Some((3.0, 11.2)),

        _ => None,
    }
}

/// Calculates the spatial keyboard distance between two characters.
/// If characters are identical, returns 0.0.
/// If one character is unknown, returns a default distance of 4.0.
/// Adjacent keys typically have a distance <= 1.45.
pub fn keyboard_distance(a: char, b: char) -> f64 {
    let a_low = a.to_ascii_lowercase();
    let b_low = b.to_ascii_lowercase();

    if a_low == b_low {
        return 0.0;
    }

    // Special case for German QWERTZ vs US QWERTY Z/Y layout swap
    if (a_low == 'z' && b_low == 'y') || (a_low == 'y' && b_low == 'z') {
        return 0.5;
    }

    // Special case for German Umlauts to base vowels
    if (a_low == 'ä' && b_low == 'a') || (a_low == 'a' && b_low == 'ä')
        || (a_low == 'ö' && b_low == 'o') || (a_low == 'o' && b_low == 'ö')
        || (a_low == 'ü' && b_low == 'u') || (a_low == 'u' && b_low == 'ü')
        || (a_low == 'ß' && b_low == 's') || (a_low == 's' && b_low == 'ß')
    {
        return 0.3;
    }

    match (key_coords(a_low), key_coords(b_low)) {
        (Some((r1, c1)), Some((r2, c2))) => {
            let dr = r1 - r2;
            let dc = c1 - c2;
            (dr * dr + dc * dc).sqrt()
        }
        _ => 4.0,
    }
}

/// Checks if character `a` and character `b` are adjacent on the keyboard ("Fettfinger" distance <= 1.45).
pub fn is_adjacent_key(a: char, b: char) -> bool {
    keyboard_distance(a, b) <= 1.45
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjacent_keys() {
        // 's' is next to 'a', 'd', 'w', 'e', 'x', 'z'
        assert!(is_adjacent_key('s', 'a'));
        assert!(is_adjacent_key('s', 'd'));
        assert!(is_adjacent_key('s', 'w'));
        assert!(is_adjacent_key('s', 'e'));
        assert!(is_adjacent_key('s', 'x'));

        // 't' is next to 'r', 'z', 'f', 'g'
        assert!(is_adjacent_key('t', 'r'));
        assert!(is_adjacent_key('t', 'f'));
        assert!(is_adjacent_key('t', 'g'));

        // 'm' is next to 'n', 'k', 'j', ','
        assert!(is_adjacent_key('m', 'n'));
        assert!(is_adjacent_key('m', 'k'));
        assert!(is_adjacent_key('m', 'j'));
    }

    #[test]
    fn test_non_adjacent_keys() {
        // 'a' is far from 'p', 'l', 'm'
        assert!(!is_adjacent_key('a', 'p'));
        assert!(!is_adjacent_key('a', 'l'));
        assert!(!is_adjacent_key('a', 'm'));
        assert!(keyboard_distance('a', 'p') > 5.0);
    }

    #[test]
    fn test_qwerty_qwertz_swap() {
        assert!(is_adjacent_key('z', 'y'));
        assert_eq!(keyboard_distance('z', 'y'), 0.5);
    }

    #[test]
    fn test_umlaut_distance() {
        assert!(keyboard_distance('ä', 'a') <= 0.5);
        assert!(keyboard_distance('ü', 'u') <= 0.5);
    }
}
