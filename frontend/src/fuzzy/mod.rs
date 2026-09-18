//! Generalized Fuzzy Matching & Contextual Semantic Scoring Engine.
//!
//! Provides configurable string fuzzy matching (with initialisms, umlauts, adjacent-key typo tolerance)
//! and a composable search engine with weighted field extractors and score boost/debuff modifiers.

pub mod boost;
pub mod config;
pub mod engine;
pub mod keyboard;
pub mod matcher;

pub use boost::{apply_modifiers, ModifierKind, ScoreModifier};
pub use config::FuzzyConfig;
pub use engine::{FieldExtractor, ScoredResult, SearchEngine, SearchEngineBuilder};
pub use keyboard::{is_adjacent_key, keyboard_distance};
pub use matcher::{extract_initialisms, fuzzy_score, normalize_string};
