//! What an id may look like, wherever one arrives from outside (an address, the browser's
//! storage): ids end up in links and queries.

/// What a module id may look like wherever one arrives from outside (a URL, the browser's
/// storage): it ends up in links and queries.
pub fn is_module_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// What a program id may look like where one arrives from the browser's storage (`079-82-2008`,
/// `G29-82-2025`): three to five parts of one to eight ASCII letters or digits, joined by `-`.
pub fn is_program_id(id: &str) -> bool {
    let part = |part: &str| (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric());
    (3..=5).contains(&id.split('-').count()) && id.split('-').all(part)
}

/// The most module ids one Studienplan query takes: the store's cap of planned modules in all
/// semesters (`folia_plans::studyplan`), so a whole plan is one answer.
pub const MAX_PLANNED: usize = 400;
