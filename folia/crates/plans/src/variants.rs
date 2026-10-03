//! The study plans of a program as its regulation prints them: one plan per study direction, or a
//! core plan with pages that each fill one of its rows („Studienplan · Seite 7" … for the row
//! „gem. der gewählten ingenieurwissenschaftlichen Studienrichtung"), what each plan comes to, and
//! the catalog query a row of a plan stands for.
//!
//! The program page drew this for itself until the Studienplan needed the same reading: its import
//! takes a plan's rows, and its placeholders find their row again and link the catalog the way the
//! program page does. Everything here is a pure function of the rows
//! `queries::program_plan_entries` and `program_plan_totals` return, so it runs in the browser and
//! on the server alike.

use serde::{Deserialize, Serialize};
use folia_locale::Locale;
use folia_model::labels::ModuleKind;
use folia_model::rows_detail::{PlanEntry, PlanTotal};
use folia_routes::filter::{CatalogQuery, KindFilter, ProgramRelation, ProgramScope};
use folia_routes::url;

use crate::areas::CatalogArea;
use crate::plan;

/// One study plan of a program. Most programs have exactly one; where the regulations print one
/// plan per study direction, each is its own plan with its own semesters and its own sum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanVariant {
    /// What the chips say: the name of the plan without its boilerplate; for a plan without a
    /// name, „Regelstudienplan" in the language `plan_variants` was asked for.
    pub label: String,
    /// The name as the regulations print it (the title of the chip): the rows' `specialization`,
    /// empty for the one unnamed plan. It is what identifies a plan in a visitor's store, never
    /// its place among the others.
    pub full: String,
    /// The last semester the plan names.
    pub semesters: i64,
    /// What the whole plan comes to: the regulation's own sums where it prints them, else what
    /// its rows add up to. A plan with elective budgets („10–24 LP") has no other way of saying
    /// it — three such rows are anything between 30 and 72 LP. The two ends differ only where the
    /// regulation prints a span for a semester instead of a number.
    pub credits: f64,
    pub credits_max: f64,
    /// Whether the regulation states `credits` itself.
    pub stated: bool,
    pub entries: Vec<PlanEntry>,
    /// The sums the regulation prints over these rows, each with the rows it counts.
    pub totals: Vec<PlanTotal>,
}

impl PlanVariant {
    /// What the plan states for exactly these semesters, if it states anything.
    pub fn stated_for(&self, from: i64, to: i64) -> Option<(f64, f64)> {
        plan::stated_for_span(&self.totals.iter().collect::<Vec<_>>(), from, to)
    }

    /// What one semester holds: the regulation's own line where it prints one, else the rows the
    /// plan puts into that semester alone.
    pub fn semester_credits(&self, semester: i64) -> Option<f64> {
        self.stated_for(semester, semester).map(|(low, _)| low).or_else(|| {
            let sum: f64 = self
                .entries
                .iter()
                .filter(|entry| plan::semester_span(entry) == Some((semester, semester)))
                .map(|entry| entry.credits.or(entry.min_credits).unwrap_or(0.0))
                .sum();
            (sum > 0.0).then_some(sum)
        })
    }

    /// The sum that ties a row to the rows it is chosen with, where the regulation prints one,
    /// together with those rows: their number in this plan and their name.
    pub fn choice_for(&self, entry: &PlanEntry) -> Option<Choice> {
        let total = plan::choice_of(&self.totals.iter().collect::<Vec<_>>(), entry)?.clone();
        let with = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, other)| other.ord != entry.ord && total.entries.contains(&other.ord))
            .map(|(i, other)| (i + 1, other.module_name.clone()))
            .collect();
        Some(Choice { total, with })
    }
}

/// What a row of the plan shares with the rows it is chosen together with. „Komplex Praktische
/// Informatik, 10–24 LP" says nothing on its own; the regulation's line over it and its two
/// neighbours („Summe Komplexe des Fachstudiums 44") is what makes the three add up.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub total: PlanTotal,
    /// The other rows the sum counts: their number in this plan and their name.
    pub with: Vec<(usize, String)>,
}

/// Splits the rows of the plan into the plans they were printed as, and gives each the sums the
/// regulation prints over its rows. The order is the order of the document; rows without a name
/// of their own form one unnamed plan. The chips are labelled in `locale`.
pub fn plan_variants(entries: &[PlanEntry], totals: &[PlanTotal], locale: Locale) -> Vec<PlanVariant> {
    let mut plans: Vec<PlanVariant> = Vec::new();
    for entry in entries {
        let full = entry.specialization.clone().unwrap_or_default();
        let plan = match plans.iter_mut().find(|plan| plan.full == full) {
            Some(plan) => plan,
            None => {
                plans.push(PlanVariant {
                    label: String::new(),
                    full: full.clone(),
                    semesters: 0,
                    credits: 0.0,
                    credits_max: 0.0,
                    stated: false,
                    entries: Vec::new(),
                    totals: plan::totals_of_variant(totals, &full).into_iter().cloned().collect(),
                });
                match plans.last_mut() {
                    Some(plan) => plan,
                    None => continue,
                }
            }
        };
        if let Some((_, to)) = plan::semester_span(entry) {
            plan.semesters = plan.semesters.max(to);
        }
        let own = entry.credits.or(entry.min_credits).unwrap_or(0.0);
        plan.credits += own;
        plan.credits_max += own;
        plan.entries.push(entry.clone());
    }
    // What the rows add up to is a lower bound wherever they are budgets; the regulation's own
    // sums are the plan itself and win.
    for plan in plans.iter_mut() {
        if let Some((low, high)) = plan::stated_credits(&plan.totals.iter().collect::<Vec<_>>()) {
            plan.credits = low;
            plan.credits_max = high;
            plan.stated = true;
        }
    }
    plans.truncate(url::MAX_PLAN_VARIANTS);
    // The chips say what tells the plans apart, which only all of them together can say.
    let labels = tell_plans_apart(&plans.iter().map(|plan| plan.full.clone()).collect::<Vec<_>>(), locale);
    for (plan, label) in plans.iter_mut().zip(labels) {
        plan.label = label;
    }
    plans
}

/// The name of a plan without the words every plan of this program carries anyway. The captions
/// of the regulations differ in one place only — „Regelstudienplan der Studienrichtungen **MIT
/// und EET** im grundständigen Studium" — so a chip says „MIT und EET" and keeps the whole
/// caption as its title. Nothing is invented: where the names do not differ, the name stays. A
/// plan without a name is called „Regelstudienplan" in `locale`.
pub fn tell_plans_apart(names: &[String], locale: Locale) -> Vec<String> {
    let stripped: Vec<&str> = names.iter().map(|name| strip_plan_boilerplate(name)).collect();
    let words: Vec<Vec<&str>> = stripped.iter().map(|name| name.split_whitespace().collect()).collect();
    let shortest = words.iter().map(Vec::len).min().unwrap_or(0);
    if names.len() < 2 || shortest == 0 {
        return stripped.iter().map(|name| clip_plan_name(name, locale)).collect();
    }
    let same_at = |i: usize| {
        let first = words.first().and_then(|first| first.get(i));
        words.iter().all(|name| name.get(i) == first)
    };
    fn from_end<'a>(name: &[&'a str], i: usize) -> Option<&'a str> {
        name.len().checked_sub(i + 1).and_then(|at| name.get(at)).copied()
    }
    let same_from_end = |i: usize| {
        let first = words.first().and_then(|first| from_end(first, i));
        words.iter().all(|name| from_end(name, i) == first)
    };
    let lead = (0..shortest).take_while(|i| same_at(*i)).count();
    // Words at the end are only boilerplate when there are several of them („im grundstaendigen
    // Studium"); a single one usually belongs to the name („Konstruktiver *Ingenieurbau*").
    let tail = match (0..shortest.saturating_sub(lead)).take_while(|i| same_from_end(*i)).count() {
        1 => 0,
        several => several,
    };
    words
        .iter()
        .zip(stripped.iter())
        .map(|(name, whole)| {
            let rest: Vec<&str> = name.iter().skip(lead).take(name.len().saturating_sub(lead + tail)).copied().collect();
            match rest.is_empty() {
                true => clip_plan_name(whole, locale),
                false => clip_plan_name(rest.join(" ").trim_matches(|c: char| c == '–' || c == '-' || c == ',' || c == ';' || c.is_whitespace()), locale),
            }
        })
        .collect()
}

/// The caption of a plan without the words every such caption starts with.
pub fn strip_plan_boilerplate(name: &str) -> &str {
    let name = name.trim();
    for prefix in [
        "Regelstudienplan der Studienrichtungen ",
        "Regelstudienplan der Studienrichtung ",
        "Regelstudienplan für das ",
        "Regelstudienplans für das ",
        "Regelstudienplan ",
        "Studienplan · ",
        "Studienplan ",
        "Studienrichtung ",
    ] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return rest.trim();
        }
    }
    name
}

/// Long captions are cut at a word, never in the middle of one; the chip shortens what is still
/// too wide for it, and the whole caption is its title. No caption at all is „Regelstudienplan"
/// in `locale`.
pub fn clip_plan_name(name: &str, locale: Locale) -> String {
    let name = name.trim();
    if name.is_empty() {
        return crate::i18n::texts(locale).unnamed_plan.to_string();
    }
    if name.chars().count() <= 72 {
        return name.to_string();
    }
    let cut = name.char_indices().nth(72).map(|(at, _)| at).unwrap_or(name.len());
    let head = name.get(..cut).unwrap_or(name);
    let head = head.rsplit_once(' ').map(|(start, _)| start).unwrap_or(head);
    format!("{}…", head.trim_end_matches([',', ';', '(']).trim())
}

/// A row of the plan as a question to the catalog, and what the link to it says: the FÜS list for
/// a FÜS row; a text search by its name for a row that means one module the catalog does not know
/// under that name; the areas its name points at (`plan::areas_for_row`, all of them where it
/// means several); else every elective of the program. The program page links a row of its plan
/// this way, and a placeholder of the Studienplan („Modul finden") does too. `areas` are the
/// program's (`pages::catalog_areas`); `program_slug` is what the catalog's URL carries. What
/// the link says is in `locale`, but for the names of the data (a row's, an area's).
pub fn row_query(program_slug: &str, variant: &PlanVariant, entry: &PlanEntry, areas: &[CatalogArea], locale: Locale) -> (CatalogQuery, String) {
    let texts = crate::i18n::texts(locale);
    let base = ProgramScope { program_slug: program_slug.to_string(), ..Default::default() };
    let scoped = |scope: ProgramScope| CatalogQuery { program: Some(scope), ..Default::default() };
    if plan::is_fues(entry) {
        return (scoped(ProgramScope { relation: ProgramRelation::Fues, ..base }), texts.fues_list.to_string());
    }
    if plan::is_single_module(entry) {
        return (CatalogQuery { text: entry.module_name.clone(), ..Default::default() }, entry.module_name.clone());
    }
    let found = plan::areas_for_row(entry, &variant.full, areas, &variant.entries);
    match found.areas.as_slice() {
        [] => (scoped(ProgramScope { kinds: vec![KindFilter::Stated(ModuleKind::Elective)], ..base }), texts.program_electives.to_string()),
        [one] => (scoped(ProgramScope { areas: vec![one.id], ..base }), one.name().to_string()),
        several => (scoped(ProgramScope { areas: several.iter().map(|area| area.id).collect(), ..base }), (texts.areas)(several.len())),
    }
}

/// The plan printed under this caption (`""` for the unnamed plan). A visitor's store keeps the
/// caption, not the plan's place among the others, which moves when the reader of the regulations
/// finds one plan more or less; space around it does not count, since the store trims its fields.
/// `None` where no plan carries the caption any more: the caller decides what stands in.
pub fn variant_for<'a>(variants: &'a [PlanVariant], caption: &str) -> Option<&'a PlanVariant> {
    variants.iter().find(|variant| variant.full.trim() == caption.trim())
}

/// A page that fills one row of a larger plan: variant `page` supplements `core` through the
/// core's row `ord` (a `PlanEntry::ord`; `core` and `page` are positions in `plan_variants`'
/// result).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Supplement {
    pub core: usize,
    pub ord: i64,
    pub page: usize,
}

/// Which plans fill a row of another one. Some regulations print the plan of the whole study
/// („Studienplan · Seite 5", 180 LP) with one row of 60 LP over semesters 1–6 („gem. der gewählten
/// ingenieurwissenschaftlichen Studienrichtung …"), and one page per direction that says what those
/// 60 LP are („Seite 7" Produktionstechnik … „Seite 11"). Nothing in the data says so; the numbers
/// do. V supplements C when V is not C, C comes to more than V, and C has a row that names no
/// module and is no FÜS row, states one number of credits equal to what V comes to, and spans
/// (`plan::semester_span`) every semester V names. Without „C comes to more" a 60-LP page whose one
/// row is a 60-LP elective („Seite 10" of 370-82-2019, Bauingenieurwesen) would read as the core of
/// its sister pages. A page that could fill two rows of one core fills the first of them. Core
/// plans are those that supplement nothing. Ordered by core, then page, both in document order.
pub fn supplements(variants: &[PlanVariant]) -> Vec<Supplement> {
    let mut found = Vec::new();
    for (core, whole) in variants.iter().enumerate() {
        for (page, part) in variants.iter().enumerate() {
            if page == core || part.credits <= 0.0 || whole.credits - part.credits < 0.01 {
                continue;
            }
            let Some((first, last)) = semesters_of(part) else {
                continue;
            };
            let row = whole.entries.iter().find(|row| {
                row.module_id.is_none()
                    && !plan::is_fues(row)
                    && one_number(row).is_some_and(|credits| (credits - part.credits).abs() < 0.01)
                    && plan::semester_span(row).is_some_and(|(from, to)| from <= first && last <= to)
            });
            if let Some(row) = row {
                found.push(Supplement { core, ord: row.ord, page });
            }
        }
    }
    found
}

/// The first and the last semester a plan names; `None` for a plan whose rows name none.
fn semesters_of(variant: &PlanVariant) -> Option<(i64, i64)> {
    variant.entries.iter().filter_map(plan::semester_span).fold(None, |range, (from, to)| match range {
        None => Some((from, to)),
        Some((first, last)) => Some((first.min(from), last.max(to))),
    })
}

/// The credits of a row where it states one number („60"), not a range („10–24") or nothing: the
/// reading of `plan::credits_of`.
fn one_number(entry: &PlanEntry) -> Option<f64> {
    match (entry.credits, entry.min_credits, entry.max_credits) {
        (Some(credits), _, _) => Some(credits),
        (None, Some(min), Some(max)) if min != max => None,
        (None, Some(value), _) | (None, None, Some(value)) => Some(value),
        (None, None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use folia_model::labels::Code;
    use folia_query as queries;
    use folia_routes::url::CatalogUrl;

    use crate::area_fixtures::{row, self as real};
    use super::*;

    #[test]
    fn the_chips_say_what_tells_the_plans_apart() {
        let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
        assert_eq!(
            tell_plans_apart(&names(&[
                "Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium",
                "Regelstudienplan der Studienrichtungen PA und IoT im grundständigen Studium",
            ]), Locale::De),
            vec!["MIT und EET", "PA und IoT"],
        );
        assert_eq!(
            tell_plans_apart(&names(&[
                "Regelstudienplan Bachelor of Science – grundlagenorientiert (180 LP) Studienrichtung Konstruktiver Ingenieurbau",
                "Regelstudienplan Bachelor of Science – grundlagenorientiert (180 LP) Studienrichtung Allgemeiner Ingenieurbau",
                "Regelstudienplan Bachelor of Science – praxisorientiert (240 LP) Studienrichtung Konstruktiver Ingenieurbau",
            ]), Locale::De),
            vec![
                "grundlagenorientiert (180 LP) Studienrichtung Konstruktiver Ingenieurbau",
                "grundlagenorientiert (180 LP) Studienrichtung Allgemeiner Ingenieurbau",
                "praxisorientiert (240 LP) Studienrichtung Konstruktiver Ingenieurbau",
            ],
        );
        // One plan keeps its name, and a plan without one is still called what it is.
        assert_eq!(tell_plans_apart(&names(&["Regelstudienplan"]), Locale::De), vec!["Regelstudienplan"]);
        assert_eq!(tell_plans_apart(&names(&[""]), Locale::De), vec!["Regelstudienplan"]);
        // What the regulation calls a plan is its name in every language; only the missing one is
        // said in the page's.
        assert_eq!(tell_plans_apart(&names(&["", "Regelstudienplan"]), Locale::En), vec!["Standard study plan", "Regelstudienplan"]);
        // Names that do not differ are not cut down to nothing.
        assert_eq!(tell_plans_apart(&names(&["Studienplan · Seite 5", "Studienplan · Seite 5"]), Locale::De), vec!["Seite 5", "Seite 5"]);
        // A plan that differs in one word keeps the word, not only what is around it.
        assert_eq!(
            tell_plans_apart(&names(&["Regelstudienplan Studienrichtung Konstruktiver Ingenieurbau", "Regelstudienplan Studienrichtung Allgemeiner Ingenieurbau"]), Locale::De),
            vec!["Konstruktiver Ingenieurbau", "Allgemeiner Ingenieurbau"],
        );
    }

    /// A row of a plan in one semester or over `span`, with `credits` or a `range` of them.
    fn plan_row(ord: i64, name: &str, semester: Option<i64>, span: Option<(i64, i64)>, credits: Option<f64>, range: Option<(f64, f64)>) -> PlanEntry {
        PlanEntry {
            ord,
            semester,
            start_semester: span.map(|(from, _)| from),
            end_semester: span.map(|(_, to)| to),
            semester_span: span.map(|(from, to)| format!("{from}-{to}")),
            credits,
            min_credits: range.map(|(min, _)| min),
            max_credits: range.map(|(_, max)| max),
            ..row(name, 1, None, None)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn total(ord: i64, label: &str, scope: &str, from: i64, to: i64, credits: f64, min: f64, max: f64, entries: Vec<i64>) -> PlanTotal {
        PlanTotal {
            ord,
            label: label.to_string(),
            scope: Code::parse(scope),
            specialization: None,
            start_semester: from,
            end_semester: to,
            credits,
            credits_max: credits,
            min_credits: min,
            max_credits: max,
            is_choice: max - min > 0.01,
            entry_count: entries.len() as i64,
            entries,
        }
    }

    // Informatik B.Sc. 2008: three rows of „10–24 LP" in the last two semesters, which the plan
    // prints as one merged column. Adding the rows up gave 166 LP and left those semesters empty;
    // the regulation's own lines say 180, and 60 for the two of them together.
    #[test]
    fn what_a_plan_adds_up_to_is_what_the_regulation_states() {
        let entries = vec![
            plan_row(1, "Entwicklung von Softwaresystemen", Some(1), None, Some(8.0), None),
            plan_row(2, "Komplex Grundlagen der Informatik", None, Some((5, 6)), None, Some((10.0, 24.0))),
            plan_row(3, "Komplex Praktische Informatik", None, Some((5, 6)), None, Some((10.0, 24.0))),
            plan_row(4, "Komplex Angewandte und Technische Informatik", None, Some((5, 6)), None, Some((10.0, 24.0))),
            plan_row(5, "Bachelor-Arbeit", None, Some((5, 6)), Some(12.0), None),
        ];
        let totals = vec![
            total(1, "Summe Komplexe des Fachstudiums", "section", 5, 6, 44.0, 30.0, 72.0, vec![2, 3, 4]),
            total(2, "Summe Studium", "plan", 1, 1, 8.0, 8.0, 8.0, vec![1]),
            total(3, "Summe Studium", "plan", 5, 6, 56.0, 42.0, 84.0, vec![2, 3, 4, 5]),
        ];
        // Adding the rows up gives 8 + 10 + 10 + 10 + 12 = 50; the plan states 8 + 56.
        let plain = plan_variants(&entries, &[], Locale::De);
        assert_eq!(plain[0].credits, 50.0);
        assert!(!plain[0].stated);
        let plans = plan_variants(&entries, &totals, Locale::De);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].credits, 64.0);
        assert!(plans[0].stated);

        // The semesters the plan sums together have one figure between them, not none each.
        let plan = &plans[0];
        assert_eq!(plan.semester_credits(1), Some(8.0));
        assert_eq!(plan.semester_credits(5), None, "5 and 6 share one sum and neither has one of its own");
        assert_eq!(plan.stated_for(5, 6), Some((56.0, 56.0)));

        // Where the plan also sums those semesters one by one, the finer statement says more.
        let mut finer = totals.clone();
        finer.push(total(4, "Summe Studium", "plan", 5, 5, 26.0, 20.0, 60.0, vec![2, 3, 4]));
        finer.push(total(5, "Summe Studium", "plan", 6, 6, 30.0, 12.0, 52.0, vec![5]));
        let plans = plan_variants(&entries, &finer, Locale::De);
        assert_eq!(plans[0].semester_credits(5), Some(26.0));
        // The whole plan is still read by the widest line it prints.
        assert_eq!(plans[0].credits, 64.0);

        // A row with a range knows the rows it is chosen with; one with a fixed value does not.
        let choice = plan.choice_for(&entries[1]).expect("the elective row is tied to the others");
        assert_eq!(choice.total.credits, 44.0);
        assert_eq!(choice.with.iter().map(|(row, _)| *row).collect::<Vec<_>>(), vec![3, 4]);
        assert!(plan.choice_for(&entries[4]).is_none(), "the thesis has a number of its own");
        assert!(plan.choice_for(&entries[0]).is_none());
    }

    /// The row half of the program page's link into the catalog, as it was before it moved here:
    /// the same query and the same second line for each kind of row.
    #[test]
    fn a_row_asks_the_catalog_what_the_program_page_asked() {
        const SLUG: &str = "bachelor-informatik-2008";
        let areas = real::areas(real::INFORMATIK_BSC);
        let variant = plan_variants(&real::informatik_bsc_rows(), &[], Locale::De).remove(0);
        let id = |label: &str| areas.iter().find(|area| area.label == label).map(|area| area.id).unwrap();
        let scope = ProgramScope { program_slug: SLUG.to_string(), ..Default::default() };
        let scoped = |scope: ProgramScope| CatalogQuery { program: Some(scope), ..Default::default() };
        let ask = |name: &str, kind: Option<ModuleKind>| row_query(SLUG, &variant, &row(name, 3, kind, None), &areas, Locale::De);
        let path = |query: CatalogQuery| CatalogUrl { query, ..Default::default() }.path();

        // FÜS: the program's FÜS list, by the name as well as by the kind.
        let fues = (scoped(ProgramScope { relation: ProgramRelation::Fues, ..scope.clone() }), "FÜS-Liste des Studiengangs".to_string());
        assert_eq!(ask("Fachübergreifendes Studium", Some(ModuleKind::Elective)), fues);
        assert_eq!(ask("Modul", Some(ModuleKind::Fues)), fues);
        assert_eq!(path(fues.0), "/catalog?program=bachelor-informatik-2008&list=fues");

        // One module the catalog does not know under this name: a search by the name, anywhere.
        let single = ask("Bachelor-Arbeit", Some(ModuleKind::Thesis));
        assert_eq!(single, (CatalogQuery { text: "Bachelor-Arbeit".to_string(), ..Default::default() }, "Bachelor-Arbeit".to_string()));
        assert_eq!(path(single.0), "/catalog?q=Bachelor-Arbeit");

        // One area: that area, named.
        let one = ask("Komplex Praktische Informatik", Some(ModuleKind::Elective));
        assert_eq!(one, (scoped(ProgramScope { areas: vec![id("Praktische Informatik")], ..scope.clone() }), "Praktische Informatik".to_string()));

        // Several areas: all of them, counted.
        let several = ask("Anwendungsfach", Some(ModuleKind::Elective));
        let meant: Vec<i64> = ["Mathematik", "Maschinenbau / Elektrotechnik", "Wirtschaftswissenschaften", "Bauingenieurwesen", "Physik"].iter().map(|label| id(label)).collect();
        assert_eq!(several, (scoped(ProgramScope { areas: meant, ..scope.clone() }), "5 Bereiche".to_string()));

        // None: every elective of the program.
        let none = ask("Wahlpflichtmodul 3", Some(ModuleKind::Elective));
        assert_eq!(none, (scoped(ProgramScope { kinds: vec![KindFilter::Stated(ModuleKind::Elective)], ..scope }), "Wahlpflichtmodule des Studiengangs".to_string()));
        assert_eq!(path(none.0), "/catalog?program=bachelor-informatik-2008&kind=elective");

        // In English the same questions, the link's words in English and the names of the data
        // as they are.
        let english = |name: &str, kind: Option<ModuleKind>| row_query(SLUG, &variant, &row(name, 3, kind, None), &areas, Locale::En).1;
        assert_eq!(english("Fachübergreifendes Studium", Some(ModuleKind::Elective)), "FÜS list of the degree programme");
        assert_eq!(english("Anwendungsfach", Some(ModuleKind::Elective)), "5 areas");
        assert_eq!(english("Wahlpflichtmodul 3", Some(ModuleKind::Elective)), "Compulsory elective modules of the degree programme");
        assert_eq!(english("Komplex Praktische Informatik", Some(ModuleKind::Elective)), "Praktische Informatik");
        assert_eq!(english("Bachelor-Arbeit", Some(ModuleKind::Thesis)), "Bachelor-Arbeit");
    }

    #[test]
    fn a_stored_caption_finds_its_plan() {
        let entries = vec![
            PlanEntry { specialization: Some("Regelstudienplan der Studienrichtungen MIT und EET".into()), ..row("A", 1, None, None) },
            PlanEntry { specialization: Some("Regelstudienplan der Studienrichtungen PA und IoT".into()), ..row("B", 1, None, None) },
        ];
        let variants = plan_variants(&entries, &[], Locale::De);
        assert_eq!(variant_for(&variants, "Regelstudienplan der Studienrichtungen PA und IoT").map(|v| v.label.as_str()), Some("PA und IoT"));
        assert_eq!(variant_for(&variants, " Regelstudienplan der Studienrichtungen MIT und EET ").map(|v| v.label.as_str()), Some("MIT und EET"));
        assert!(variant_for(&variants, "Regelstudienplan").is_none(), "no plan of another name stands in");
        let unnamed = plan_variants(&[row("A", 1, None, None)], &[], Locale::De);
        assert_eq!(variant_for(&unnamed, "").map(|v| v.full.as_str()), Some(""));
    }

    #[test]
    fn a_page_fills_the_row_of_its_size_and_semesters() {
        let core = |entry: PlanEntry| PlanEntry { specialization: Some("Studienplan · Seite 5".into()), ..entry };
        let page = |n: u8, entry: PlanEntry| PlanEntry { specialization: Some(format!("Studienplan · Seite {n}")), ..entry };
        let linked = |ord: i64, semester: i64, credits: f64| PlanEntry { module_id: Some(format!("1{ord:04}")), ..plan_row(ord, "Modul", Some(semester), None, Some(credits), None) };
        let entries = vec![
            core(linked(1, 1, 60.0)),
            core(plan_row(2, "Wahlpflicht Wirtschaftswissenschaften", None, Some((4, 6)), Some(18.0), None)),
            core(plan_row(3, "gem. der gewählten Studienrichtung", None, Some((1, 6)), Some(60.0), None)),
            core(plan_row(4, "Modul aus dem FÜS-Katalog", None, Some((1, 6)), Some(42.0), None)),
            page(7, linked(10, 1, 30.0)),
            page(7, plan_row(11, "Wahlpflicht Produktionstechnik", None, Some((4, 6)), Some(30.0), None)),
            // A page of one 60-LP row is not the core of the others.
            page(10, plan_row(20, "Wahlpflicht Bauingenieurwesen", None, Some((1, 6)), Some(60.0), None)),
            // A page reaching into a semester the row does not span fills nothing.
            page(12, linked(30, 7, 60.0)),
            // A page of the FÜS row's size: that row is filled from the FÜS list, not by a page.
            page(13, linked(40, 2, 42.0)),
        ];
        let variants = plan_variants(&entries, &[], Locale::De);
        assert_eq!(variants.iter().map(|v| v.credits).collect::<Vec<_>>(), vec![180.0, 60.0, 60.0, 60.0, 42.0]);
        assert_eq!(supplements(&variants), vec![Supplement { core: 0, ord: 3, page: 1 }, Supplement { core: 0, ord: 3, page: 2 }]);
        // A range is no one number.
        let ranged = vec![
            core(plan_row(3, "gem. der gewählten Studienrichtung", None, Some((1, 6)), None, Some((30.0, 60.0)))),
            core(linked(1, 1, 120.0)),
            page(7, linked(10, 1, 60.0)),
        ];
        assert!(supplements(&plan_variants(&ranged, &[], Locale::De)).is_empty());
        assert!(supplements(&[]).is_empty());
    }

    /// The four programs whose regulation prints a core plan and one page per study direction for
    /// one of its rows (design C.16), on the pinned snapshot; on any other, what `supplements`
    /// finds still keeps to its rule.
    #[test]
    fn supplements_fill_a_core_row() {
        let pinned = folia_test_support::studyplan_db("supplements_fill_a_core_row");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(folia_test_support::open);
        // (program, core caption, row, its credits, its span, the pages that fill it)
        type Found = (String, String, i64, f64, (i64, i64), Vec<String>);
        let mut found: Vec<Found> = Vec::new();
        for program in queries::programs(&db).unwrap().into_iter().filter(|program| program.has_plan) {
            let entries = queries::program_plan_entries(&db, &program.id).unwrap();
            let totals = queries::program_plan_totals(&db, &program.id).unwrap();
            let variants = plan_variants(&entries, &totals, Locale::De);
            let all = supplements(&variants);
            for supplement in &all {
                let (core, page) = (&variants[supplement.core], &variants[supplement.page]);
                let row = core.entries.iter().find(|row| row.ord == supplement.ord).expect("the row is the core's");
                let span = plan::semester_span(row).expect("the row spans semesters");
                let credits = one_number(row).expect("the row states one number");
                // The rule, read again from what was found.
                assert!(row.module_id.is_none() && !plan::is_fues(row), "{}: {}", program.id, row.module_name);
                assert!(core.credits > page.credits && (credits - page.credits).abs() < 0.01, "{}: {}", program.id, page.full);
                let pages = all.iter().filter(|other| other.core == supplement.core && other.ord == supplement.ord).map(|other| variants[other.page].full.clone()).collect();
                let entry = (program.id.clone(), core.full.clone(), row.ord, credits, span, pages);
                if !found.contains(&entry) {
                    found.push(entry);
                }
            }
        }
        if !is_pinned {
            return;
        }
        // `programs` lists by name; the programs' ids read more easily in order.
        found.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
        let expected = |program: &str, core: &str, ord: i64, credits: f64, span: (i64, i64), pages: &[u8]| -> Found {
            (program.to_string(), core.to_string(), ord, credits, span, pages.iter().map(|n| format!("Studienplan · Seite {n}")).collect())
        };
        assert_eq!(
            found,
            vec![
                expected("370-82-2019", "Studienplan · Seite 5", 16, 60.0, (1, 6), &[7, 8, 9, 10, 11]),
                expected("370-82-2023", "Regelstudienplan für das grundständige Studium (Übersicht der Module, Status, Leistungspunkte (LP", 16, 60.0, (1, 6), &[18, 19, 20, 21, 22]),
                expected("370-88-2019", "Studienplan · Seite 6", 4, 54.0, (1, 3), &[9, 10, 11, 12, 13]),
                expected(
                    "G29-82-2025",
                    "Regelstudienplan für die Studienrichtungen „Wassermanagement“, „Landnutzung“, „Nachhaltigkeitsstrategien“ und „Stadt- und Regionalplanung“ und das „Studium in der Breite“.",
                    67,
                    66.0,
                    (2, 6),
                    &[9, 10],
                ),
            ],
        );
        // Programs with one plan, or with one whole plan per study direction, have none.
        for id in ["079-82-2008", "048-82-2022"] {
            assert!(!found.iter().any(|(program, ..)| program == id), "{id}");
        }
    }
}
