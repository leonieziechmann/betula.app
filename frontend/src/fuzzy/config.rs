//! Configuration for fuzzy string matching, scoring weights, and typo tolerance.

#[derive(Clone, Debug, PartialEq)]
pub struct FuzzyConfig {
    /// Whether character case matters. Default is false.
    pub case_sensitive: bool,

    /// Whether to recognize acronyms and initialisms (e.g. "TI" -> "Theoretische Informatik", "AI" -> "Artificial Intelligence").
    pub match_initialisms: bool,

    /// Whether German umlauts (ä, ö, ü, ß) are normalized (e.g. "ae" matches "ä", "ss" matches "ß").
    pub normalize_umlauts: bool,

    /// Whether typo tolerance (transpositions, adjacent keys) is enabled.
    pub allow_typos: bool,

    /// Maximum number of allowed typos per token/word.
    pub max_typos_per_word: usize,

    /// Whether multi-token queries (e.g. "inf master") match words across the target in any order.
    pub token_matching: bool,

    /// Minimum score threshold. Matches scoring lower will be discarded.
    pub min_score_threshold: Option<f64>,

    // Scoring bonuses
    pub exact_match_bonus: f64,
    pub exact_word_bonus: f64,
    pub starts_with_bonus: f64,
    pub prefix_match_bonus: f64,
    pub word_boundary_bonus: f64,
    pub consecutive_bonus: f64,
    pub initialism_match_bonus: f64,

    // Penalties
    pub adjacent_typo_penalty: f64,
    pub unrelated_typo_penalty: f64,
    pub transposition_penalty: f64,
    pub span_penalty_weight: f64,
    pub length_penalty_weight: f64,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            match_initialisms: true,
            normalize_umlauts: true,
            allow_typos: true,
            max_typos_per_word: 2,
            token_matching: true,
            min_score_threshold: Some(0.0),

            exact_match_bonus: 2000.0,
            exact_word_bonus: 500.0,
            starts_with_bonus: 300.0,
            prefix_match_bonus: 150.0,
            word_boundary_bonus: 120.0,
            consecutive_bonus: 35.0,
            initialism_match_bonus: 450.0,

            adjacent_typo_penalty: 40.0,
            unrelated_typo_penalty: 120.0,
            transposition_penalty: 30.0,
            span_penalty_weight: 2.0,
            length_penalty_weight: 0.5,
        }
    }
}

impl FuzzyConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Strict configuration: no typos, case-sensitive option, standard sequential matching.
    pub fn strict() -> Self {
        Self {
            case_sensitive: true,
            match_initialisms: false,
            normalize_umlauts: false,
            allow_typos: false,
            max_typos_per_word: 0,
            token_matching: false,
            ..Self::default()
        }
    }

    /// Fast standard configuration (ideal for high-frequency comboboxes).
    pub fn combobox() -> Self {
        Self {
            case_sensitive: false,
            match_initialisms: true,
            normalize_umlauts: true,
            allow_typos: true,
            max_typos_per_word: 1,
            token_matching: true,
            ..Self::default()
        }
    }

    pub fn case_sensitive(mut self, enabled: bool) -> Self {
        self.case_sensitive = enabled;
        self
    }

    pub fn with_initialisms(mut self, enabled: bool) -> Self {
        self.match_initialisms = enabled;
        self
    }

    pub fn with_umlaut_normalization(mut self, enabled: bool) -> Self {
        self.normalize_umlauts = enabled;
        self
    }

    pub fn with_typo_tolerance(mut self, enabled: bool) -> Self {
        self.allow_typos = enabled;
        self
    }

    pub fn with_max_typos(mut self, max_typos: usize) -> Self {
        self.max_typos_per_word = max_typos;
        self
    }

    pub fn with_token_matching(mut self, enabled: bool) -> Self {
        self.token_matching = enabled;
        self
    }

    pub fn with_threshold(mut self, threshold: Option<f64>) -> Self {
        self.min_score_threshold = threshold;
        self
    }
}
