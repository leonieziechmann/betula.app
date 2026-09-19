//! Text folding for every search of the app, so „okologie" finds „Ökologie" and „strasse"
//! finds „Straße" on the server and in the browser alike.

/// Lower case, `ß` → `ss`, umlauts and the common Latin diacritics removed.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'ä' | 'à' | 'á' | 'â' | 'ã' | 'å' => out.push('a'),
            'ö' | 'ò' | 'ó' | 'ô' | 'õ' | 'ø' => out.push('o'),
            'ü' | 'ù' | 'ú' | 'û' => out.push('u'),
            'è' | 'é' | 'ê' | 'ë' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' => out.push('i'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            'ß' => out.push_str("ss"),
            other => out.push(other),
        }
    }
    out
}

/// True if every word of `query` occurs in `text` (both folded).
pub fn matches(text: &str, query: &str) -> bool {
    let text = fold(text);
    fold(query).split_whitespace().all(|word| text.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding() {
        assert_eq!(fold("Ökologie & Straße"), "okologie & strasse");
        assert!(!matches("Umweltingenieurwesen (B.Sc.)", "umwelt bsc"));
        assert!(matches("Umweltingenieurwesen B.Sc.", "UMWELT b.sc"));
        assert!(matches("Ökologie", "okolog"));
        assert!(matches("anything", "  "));
    }
}
