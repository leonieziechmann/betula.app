//! Semantic score modifiers, boost multipliers, and debuff rules for contextual ranking.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModifierKind {
    /// Multiplies the base score by a factor (e.g. 1.35 for a +35% buff, 0.70 for a -30% debuff).
    Multiplier(f64),
    /// Adds or subtracts a fixed point value to/from the score (e.g. +100.0 or -50.0).
    Additive(f64),
}

/// A composable score modifier that evaluates an item `T` against a search context `Ctx`.
pub struct ScoreModifier<T, Ctx = ()> {
    pub name: String,
    pub evaluator: Box<dyn Fn(&T, &Ctx) -> Option<ModifierKind>>,
}

impl<T, Ctx> ScoreModifier<T, Ctx> {
    /// Creates a new score modifier with a custom dynamic evaluator.
    pub fn new<F>(name: impl Into<String>, evaluator: F) -> Self
    where
        F: Fn(&T, &Ctx) -> Option<ModifierKind> + 'static,
    {
        Self {
            name: name.into(),
            evaluator: Box::new(evaluator),
        }
    }

    /// Creates a conditional multiplier modifier (e.g. +40% boost if predicate is true).
    pub fn multiplier<P>(name: impl Into<String>, factor: f64, predicate: P) -> Self
    where
        P: Fn(&T, &Ctx) -> bool + 'static,
    {
        Self {
            name: name.into(),
            evaluator: Box::new(move |item, ctx| {
                if predicate(item, ctx) {
                    Some(ModifierKind::Multiplier(factor))
                } else {
                    None
                }
            }),
        }
    }

    /// Creates a conditional additive modifier (e.g. +150 points if predicate is true).
    pub fn additive<P>(name: impl Into<String>, amount: f64, predicate: P) -> Self
    where
        P: Fn(&T, &Ctx) -> bool + 'static,
    {
        Self {
            name: name.into(),
            evaluator: Box::new(move |item, ctx| {
                if predicate(item, ctx) {
                    Some(ModifierKind::Additive(amount))
                } else {
                    None
                }
            }),
        }
    }
}

/// Applies all matching score modifiers to a base score.
/// Multipliers and additive bonuses are accumulated sequentially.
pub fn apply_modifiers<T, Ctx>(
    base_score: f64,
    item: &T,
    ctx: &Ctx,
    modifiers: &[ScoreModifier<T, Ctx>],
) -> f64 {
    let mut score = base_score;

    for modifier in modifiers {
        if let Some(kind) = (modifier.evaluator)(item, ctx) {
            match kind {
                ModifierKind::Multiplier(m) => {
                    score *= m;
                }
                ModifierKind::Additive(a) => {
                    score += a;
                }
            }
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    struct TestModule {
        id: String,
        is_in_program: bool,
        is_mandatory: bool,
        is_completed: bool,
        turnus_soon: bool,
    }

    #[allow(dead_code)]
    struct TestContext {
        active_program: String,
    }

    #[test]
    fn test_boost_multiplier_and_debuff() {
        let module_a = TestModule {
            id: "12345".to_string(),
            is_in_program: true,
            is_mandatory: true,
            is_completed: false,
            turnus_soon: true,
        };

        let module_b = TestModule {
            id: "67890".to_string(),
            is_in_program: false,
            is_mandatory: false,
            is_completed: true,
            turnus_soon: false,
        };

        let ctx = TestContext {
            active_program: "Informatik".to_string(),
        };

        let modifiers = vec![
            // +40% boost for modules in the selected program
            ScoreModifier::multiplier("program_match", 1.40, |m: &TestModule, _: &TestContext| {
                m.is_in_program
            }),
            // +15% boost for mandatory modules
            ScoreModifier::multiplier("mandatory_buff", 1.15, |m: &TestModule, _: &TestContext| {
                m.is_mandatory
            }),
            // -40% debuff for already completed modules
            ScoreModifier::multiplier("completed_debuff", 0.60, |m: &TestModule, _: &TestContext| {
                m.is_completed
            }),
            // -20% debuff for modules not offered soon
            ScoreModifier::multiplier("turnus_debuff", 0.80, |m: &TestModule, _: &TestContext| {
                !m.turnus_soon
            }),
        ];

        let base_score = 100.0;
        let score_a = apply_modifiers(base_score, &module_a, &ctx, &modifiers);
        let score_b = apply_modifiers(base_score, &module_b, &ctx, &modifiers);

        // Module A: 100.0 * 1.40 * 1.15 = 161.0
        assert!((score_a - 161.0).abs() < 0.001);

        // Module B: 100.0 * 0.60 * 0.80 = 48.0
        assert!((score_b - 48.0).abs() < 0.001);

        assert!(score_a > score_b * 3.0);
    }
}
