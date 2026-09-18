//! Core fuzzy string matching algorithm supporting initialisms, word boundaries, umlauts, and typo tolerance.

use crate::fuzzy::config::FuzzyConfig;
use crate::fuzzy::keyboard::{is_adjacent_key, keyboard_distance};

/// Normalizes German umlauts and ligature characters if enabled.
pub fn normalize_string(s: &str, normalize_umlauts: bool, case_sensitive: bool) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        let ch = if case_sensitive { c } else { c.to_lowercase().next().unwrap_or(c) };
        if normalize_umlauts {
            match ch {
                'ä' => out.push_str("ae"),
                'ö' => out.push_str("oe"),
                'ü' => out.push_str("ue"),
                'ß' => out.push_str("ss"),
                'Ä' => out.push_str("Ae"),
                'Ö' => out.push_str("Oe"),
                'Ü' => out.push_str("Ue"),
                other => out.push(other),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Extracts initialisms / acronym letters from a target string (e.g. "Theoretische Informatik" -> "ti").
pub fn extract_initialisms(s: &str) -> String {
    let mut acronym = String::new();
    let mut take_next = true;

    for c in s.chars() {
        if c.is_alphanumeric() {
            if take_next {
                acronym.push(c.to_ascii_lowercase());
                take_next = false;
            }
        } else {
            take_next = true;
        }
    }
    acronym
}

/// Checks if character at `idx` in `chars` is preceded by a word boundary.
fn is_word_boundary(chars: &[char], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    let prev = chars[idx - 1];
    matches!(prev, ' ' | '\t' | '(' | ')' | '[' | ']' | '{' | '}' | '-' | '_' | '/' | '\\' | '.' | ':' | ',' | ';' | '&' | '+')
}

/// Computes a fuzzy match score for a single query token against a target string.
fn match_single_token(query: &str, target: &str, config: &FuzzyConfig) -> Option<f64> {
    let q_trimmed = query.trim();
    if q_trimmed.is_empty() {
        return Some(0.0);
    }

    let q_norm = if config.case_sensitive { q_trimmed.to_string() } else { q_trimmed.to_lowercase() };
    let t_norm = if config.case_sensitive { target.to_string() } else { target.to_lowercase() };

    let q_chars: Vec<char> = q_norm.chars().collect();
    let t_chars: Vec<char> = t_norm.chars().collect();

    if q_chars.is_empty() {
        return Some(0.0);
    }
    if t_chars.is_empty() {
        return None;
    }

    // 1. Exact string match
    if q_norm == t_norm {
        return Some(config.exact_match_bonus);
    }

    // 2. Initialisms / Acronym match (e.g. "ti" matching "Theoretische Informatik")
    let mut initialism_bonus = 0.0;
    if config.match_initialisms {
        let initialism = extract_initialisms(target);
        if !initialism.is_empty() {
            if initialism == q_norm {
                initialism_bonus = config.initialism_match_bonus * 1.5;
            } else if initialism.starts_with(&q_norm) && q_norm.len() >= 2 {
                initialism_bonus = config.initialism_match_bonus;
            }
        }
    }

    // 3. Exact Substring bonus
    let exact_substr = t_norm.contains(&q_norm);
    let starts_with = t_norm.starts_with(&q_norm);

    // 4. Sequential fuzzy alignment with typo resilience
    let mut q_idx = 0;
    let mut score = 0.0;
    let mut consecutive_matches = 0;
    let mut first_match_idx = None;
    let mut last_match_idx = 0;
    let mut typo_count = 0;

    let mut t_idx = 0;
    while t_idx < t_chars.len() && q_idx < q_chars.len() {
        let qc = q_chars[q_idx];
        let tc = t_chars[t_idx];

        if qc == tc {
            // Exact character match
            if first_match_idx.is_none() {
                first_match_idx = Some(t_idx);
            }
            last_match_idx = t_idx;

            let mut char_score = 10.0;
            if t_idx == 0 {
                char_score += config.prefix_match_bonus;
            } else if is_word_boundary(&t_chars, t_idx) {
                char_score += config.word_boundary_bonus;
            }

            if consecutive_matches > 0 {
                char_score += config.consecutive_bonus * (consecutive_matches as f64);
            }
            consecutive_matches += 1;
            score += char_score;
            q_idx += 1;
            t_idx += 1;
        } else if config.allow_typos && typo_count < config.max_typos_per_word {
            // Check for transposition (swap of 2 adjacent query chars: e.g. "teh" -> "the")
            if q_idx + 1 < q_chars.len() && t_idx + 1 < t_chars.len()
                && q_chars[q_idx] == t_chars[t_idx + 1]
                && q_chars[q_idx + 1] == t_chars[t_idx]
            {
                if first_match_idx.is_none() {
                    first_match_idx = Some(t_idx);
                }
                last_match_idx = t_idx + 1;

                score += 15.0 - config.transposition_penalty;
                typo_count += 1;
                consecutive_matches = 0;
                q_idx += 2;
                t_idx += 2;
                continue;
            }

            // Check for adjacent key typo ("Fettfinger" error)
            if is_adjacent_key(qc, tc) {
                if first_match_idx.is_none() {
                    first_match_idx = Some(t_idx);
                }
                last_match_idx = t_idx;

                let dist = keyboard_distance(qc, tc);
                let penalty = config.adjacent_typo_penalty * (dist / 1.45).min(1.0);
                score += 5.0 - penalty;

                typo_count += 1;
                consecutive_matches = 0;
                q_idx += 1;
                t_idx += 1;
                continue;
            }

            // Check if query character is present slightly later in target
            t_idx += 1;
            consecutive_matches = 0;
        } else {
            consecutive_matches = 0;
            t_idx += 1;
        }
    }

    // Check if entire query was matched
    if q_idx == q_chars.len() {
        if exact_substr {
            score += config.exact_word_bonus;
            if starts_with {
                score += config.starts_with_bonus;
            }
        }

        if let Some(first) = first_match_idx {
            let span = (last_match_idx - first + 1) as f64;
            score -= span * config.span_penalty_weight;
        }

        score -= (t_chars.len() as f64) * config.length_penalty_weight;
        score += initialism_bonus;

        Some(score)
    } else if initialism_bonus > 0.0 {
        // If initialism matched even if sequential char scan was incomplete
        Some(initialism_bonus - (t_chars.len() as f64) * config.length_penalty_weight)
    } else {
        None
    }
}

/// Calculates the overall fuzzy match score of a query against a target string.
/// Supports multi-token search ("inf master"), initialisms, umlauts, and typo correction.
pub fn fuzzy_score(query: &str, target: &str, config: &FuzzyConfig) -> Option<f64> {
    let q_trimmed = query.trim();
    if q_trimmed.is_empty() {
        return Some(0.0);
    }
    if target.trim().is_empty() {
        return None;
    }

    // Try direct match or multi-token match
    let mut total_score = 0.0;

    if config.token_matching && q_trimmed.contains(' ') {
        let tokens: Vec<&str> = q_trimmed.split_whitespace().collect();
        for token in &tokens {
            // First try matching raw target
            let score_direct = match_single_token(token, target, config);
            let score = match score_direct {
                Some(s) => Some(s),
                None if config.normalize_umlauts => {
                    // Try with normalized umlauts
                    let norm_target = normalize_string(target, true, config.case_sensitive);
                    let norm_token = normalize_string(token, true, config.case_sensitive);
                    match_single_token(&norm_token, &norm_target, config)
                }
                None => None,
            };

            match score {
                Some(s) => total_score += s,
                None => return None, // All tokens must match
            }
        }
        total_score += 50.0; // Multi-token coordination bonus
    } else {
        let score_direct = match_single_token(q_trimmed, target, config);
        let score = match score_direct {
            Some(s) => Some(s),
            None if config.normalize_umlauts => {
                let norm_target = normalize_string(target, true, config.case_sensitive);
                let norm_q = normalize_string(q_trimmed, true, config.case_sensitive);
                match_single_token(&norm_q, &norm_target, config)
            }
            None => None,
        };

        match score {
            Some(s) => total_score = s,
            None => return None,
        }
    }

    if let Some(threshold) = config.min_score_threshold {
        if total_score < threshold {
            return None;
        }
    }

    Some(total_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_query() {
        let cfg = FuzzyConfig::default();
        assert_eq!(fuzzy_score("", "Informatik", &cfg), Some(0.0));
        assert_eq!(fuzzy_score("   ", "Informatik", &cfg), Some(0.0));
    }

    #[test]
    fn test_exact_match() {
        let cfg = FuzzyConfig::default();
        let score = fuzzy_score("Informatik", "Informatik", &cfg);
        assert!(score.is_some());
        assert!(score.unwrap() >= 2000.0);
    }

    #[test]
    fn test_prefix_and_substring() {
        let cfg = FuzzyConfig::default();
        let score_prefix = fuzzy_score("inf", "Informatik (B.Sc.)", &cfg);
        let score_mid = fuzzy_score("inf", "Medizinische Informatik", &cfg);
        let score_none = fuzzy_score("xyz", "Informatik", &cfg);

        assert!(score_prefix.is_some());
        assert!(score_mid.is_some());
        assert!(score_none.is_none());
        assert!(score_prefix.unwrap() > score_mid.unwrap());
    }

    #[test]
    fn test_initialisms_acronyms() {
        let cfg = FuzzyConfig::default().with_initialisms(true);
        let score_ti = fuzzy_score("ti", "Theoretische Informatik", &cfg);
        let score_ai = fuzzy_score("ai", "Artificial Intelligence", &cfg);
        let score_ds = fuzzy_score("ds", "Data Science", &cfg);

        assert!(score_ti.is_some(), "TI should match Theoretische Informatik");
        assert!(score_ai.is_some(), "AI should match Artificial Intelligence");
        assert!(score_ds.is_some(), "DS should match Data Science");
    }

    #[test]
    fn test_typo_adjacent_key() {
        let cfg = FuzzyConfig::default().with_typo_tolerance(true);
        // 'o' is next to 'i', 't' is next to 'r' -> "infotmatok"
        let score_typo = fuzzy_score("infotmatik", "Informatik", &cfg);
        assert!(score_typo.is_some(), "Adjacent key typo should match");

        let score_exact = fuzzy_score("informatik", "Informatik", &cfg);
        assert!(score_exact.unwrap() > score_typo.unwrap(), "Exact should score higher than typo");
    }

    #[test]
    fn test_transposition_typo() {
        let cfg = FuzzyConfig::default().with_typo_tolerance(true);
        // "inforamtik" (am instead of ma)
        let score_transposed = fuzzy_score("inforamtik", "Informatik", &cfg);
        assert!(score_transposed.is_some(), "Transposed characters should match");
    }

    #[test]
    fn test_umlauts_normalization() {
        let cfg = FuzzyConfig::default().with_umlaut_normalization(true);
        let score_ae = fuzzy_score("einfuehrung", "Einführung in die Informatik", &cfg);
        assert!(score_ae.is_some(), "'ue' should match 'ü'");
    }

    #[test]
    fn test_multi_token_search() {
        let cfg = FuzzyConfig::default();
        let score = fuzzy_score("inf bachelor", "Informatik (Bachelor of Science)", &cfg);
        assert!(score.is_some(), "Multi-token search should match across string");
    }
}
