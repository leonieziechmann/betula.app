//! Generalized search engine combining multi-field weighted extraction, fuzzy matching, and semantic score modifiers.

use crate::fuzzy::boost::{apply_modifiers, ScoreModifier};
use crate::fuzzy::config::FuzzyConfig;
use crate::fuzzy::matcher::fuzzy_score;

/// Extracts a searchable string property from an item `T` with an associated importance weight.
pub struct FieldExtractor<T> {
    pub name: String,
    pub weight: f64,
    pub extractor: Box<dyn Fn(&T) -> Option<String>>,
}

impl<T> FieldExtractor<T> {
    pub fn new<F>(name: impl Into<String>, weight: f64, extractor: F) -> Self
    where
        F: Fn(&T) -> Option<String> + 'static,
    {
        Self {
            name: name.into(),
            weight,
            extractor: Box::new(extractor),
        }
    }

    pub fn direct<F, S>(name: impl Into<String>, weight: f64, extractor: F) -> Self
    where
        F: Fn(&T) -> S + 'static,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            weight,
            extractor: Box::new(move |item| Some(extractor(item).into())),
        }
    }
}

/// Result of a scored search query on item `T`.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoredResult<T> {
    pub item: T,
    pub base_score: f64,
    pub final_score: f64,
    pub best_field: Option<String>,
}

/// Highly configurable, reusable search engine for arbitrary item types and search contexts.
pub struct SearchEngine<T, Ctx = ()> {
    pub config: FuzzyConfig,
    pub fields: Vec<FieldExtractor<T>>,
    pub modifiers: Vec<ScoreModifier<T, Ctx>>,
}

impl<T, Ctx> SearchEngine<T, Ctx> {
    pub fn builder() -> SearchEngineBuilder<T, Ctx> {
        SearchEngineBuilder::new()
    }

    /// Performs a fuzzy search across all configured fields, applies contextual score boosts/debuffs, and sorts results by final score.
    pub fn search(&self, query: &str, items: impl IntoIterator<Item = T>, ctx: &Ctx) -> Vec<ScoredResult<T>> {
        let q_trimmed = query.trim();
        let mut results = Vec::new();

        for item in items {
            if q_trimmed.is_empty() {
                // When query is empty, all items are included with baseline score
                let final_score = apply_modifiers(0.0, &item, ctx, &self.modifiers);
                results.push(ScoredResult {
                    item,
                    base_score: 0.0,
                    final_score,
                    best_field: None,
                });
                continue;
            }

            let mut best_field_score: Option<(f64, String)> = None;

            for field in &self.fields {
                if let Some(text) = (field.extractor)(&item) {
                    if let Some(raw_score) = fuzzy_score(q_trimmed, &text, &self.config) {
                        let weighted_score = raw_score * field.weight;
                        match &best_field_score {
                            Some((cur_max, _)) if weighted_score > *cur_max => {
                                best_field_score = Some((weighted_score, field.name.clone()));
                            }
                            None => {
                                best_field_score = Some((weighted_score, field.name.clone()));
                            }
                            _ => {}
                        }
                    }
                }
            }

            if let Some((base_score, field_name)) = best_field_score {
                let final_score = apply_modifiers(base_score, &item, ctx, &self.modifiers);
                results.push(ScoredResult {
                    item,
                    base_score,
                    final_score,
                    best_field: Some(field_name),
                });
            }
        }

        // Sort descending by final score
        results.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

/// Fluent builder for constructing `SearchEngine` instances.
pub struct SearchEngineBuilder<T, Ctx = ()> {
    config: FuzzyConfig,
    fields: Vec<FieldExtractor<T>>,
    modifiers: Vec<ScoreModifier<T, Ctx>>,
}

impl<T, Ctx> Default for SearchEngineBuilder<T, Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, Ctx> SearchEngineBuilder<T, Ctx> {
    pub fn new() -> Self {
        Self {
            config: FuzzyConfig::default(),
            fields: Vec::new(),
            modifiers: Vec::new(),
        }
    }

    pub fn with_config(mut self, config: FuzzyConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_field<F, S>(mut self, name: impl Into<String>, weight: f64, extractor: F) -> Self
    where
        F: Fn(&T) -> S + 'static,
        S: Into<String>,
    {
        self.fields.push(FieldExtractor::direct(name, weight, extractor));
        self
    }

    pub fn with_optional_field<F>(mut self, name: impl Into<String>, weight: f64, extractor: F) -> Self
    where
        F: Fn(&T) -> Option<String> + 'static,
    {
        self.fields.push(FieldExtractor::new(name, weight, extractor));
        self
    }

    pub fn with_modifier(mut self, modifier: ScoreModifier<T, Ctx>) -> Self {
        self.modifiers.push(modifier);
        self
    }

    pub fn with_boost_multiplier<P>(mut self, name: impl Into<String>, factor: f64, predicate: P) -> Self
    where
        P: Fn(&T, &Ctx) -> bool + 'static,
    {
        self.modifiers.push(ScoreModifier::multiplier(name, factor, predicate));
        self
    }

    pub fn with_additive_bonus<P>(mut self, name: impl Into<String>, amount: f64, predicate: P) -> Self
    where
        P: Fn(&T, &Ctx) -> bool + 'static,
    {
        self.modifiers.push(ScoreModifier::additive(name, amount, predicate));
        self
    }

    pub fn build(self) -> SearchEngine<T, Ctx> {
        SearchEngine {
            config: self.config,
            fields: self.fields,
            modifiers: self.modifiers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct ModuleItem {
        id: String,
        title_de: String,
        title_en: String,
        responsible: String,
        is_mandatory: bool,
        program_id: String,
    }

    struct SearchCtx {
        active_program_id: String,
    }

    #[test]
    fn test_search_engine_ranking_and_boosts() {
        let items = vec![
            ModuleItem {
                id: "12345".to_string(),
                title_de: "Theoretische Informatik 1".to_string(),
                title_en: "Theoretical Computer Science 1".to_string(),
                responsible: "Prof. Dr. Muster".to_string(),
                is_mandatory: true,
                program_id: "prog_inf".to_string(),
            },
            ModuleItem {
                id: "54321".to_string(),
                title_de: "Informatik im Maschinenbau".to_string(),
                title_en: "Computer Science for Mechanical Engineering".to_string(),
                responsible: "Prof. Dr. Schmidt".to_string(),
                is_mandatory: false,
                program_id: "prog_mb".to_string(),
            },
            ModuleItem {
                id: "99999".to_string(),
                title_de: "Angewandte Mathematik".to_string(),
                title_en: "Applied Mathematics".to_string(),
                responsible: "Prof. Dr. Informatik-Fan".to_string(),
                is_mandatory: false,
                program_id: "prog_math".to_string(),
            },
        ];

        let engine = SearchEngine::<ModuleItem, SearchCtx>::builder()
            .with_config(FuzzyConfig::default().with_initialisms(true).with_typo_tolerance(true))
            .with_field("title_de", 1.0, |m| m.title_de.clone())
            .with_field("title_en", 0.9, |m| m.title_en.clone())
            .with_field("code", 1.5, |m| m.id.clone())
            .with_field("responsible", 0.8, |m| m.responsible.clone())
            // Boost for active study program
            .with_boost_multiplier("program_match", 1.5, |m, ctx: &SearchCtx| m.program_id == ctx.active_program_id)
            // Small buff for mandatory
            .with_boost_multiplier("mandatory_buff", 1.2, |m, _| m.is_mandatory)
            .build();

        let ctx = SearchCtx {
            active_program_id: "prog_inf".to_string(),
        };

        // Query "TI" (Initialism test)
        let results_ti = engine.search("TI", items.clone(), &ctx);
        assert!(!results_ti.is_empty());
        assert_eq!(results_ti[0].item.id, "12345");

        // Query "Informatik"
        let results_inf = engine.search("Informatik", items, &ctx);
        assert_eq!(results_inf.len(), 3);
        // The CS mandatory module with active program should be ranked first due to boosts!
        assert_eq!(results_inf[0].item.id, "12345");
    }
}
