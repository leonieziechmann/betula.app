//! The Regelstudienplan and the Wahlpflicht as „Mein Studium" shows them (owner, 2026-10-14:
//! concept A, „Ansichten": the program's pages become views of the study, with the student's stand
//! in them). Every row of the plan with where it stands in the study (`PlanLine`), and what the plan
//! leaves to choose (`Choice`): its rows without a module, those of one name in one area together
//! („Anwendungsfach" in the 3. and in the 4. FS), each with the areas of the module tree its
//! modules are listed in.
//!
//! Which module fills which row is never guessed (`studyplan::Planned::fills`): a row without a
//! module stands as the student put it — its placeholders, the modules that fill them, its mark as
//! done. What the student planned or passed of a choice's lists is said of the choice as a whole
//! („8 von 12 LP"), as the progress counts a module towards an area by where the tree places it
//! (`study::study`). A module counts towards one choice at most: the first, in the order of the
//! plan, that lists it and has room left.

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Code, ModuleKind};
use folia_routes::url;

use crate::areas::CatalogArea;
use crate::plan;
use crate::study::{self, AreaKind, Input, Offer, PlanRow, Study, EPSILON};

/// Where a row of the plan stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    /// Passed, in that semester.
    Passed(SemesterKey),
    /// Planned into that semester, the first from the current one on.
    Planned(SemesterKey),
    /// Not passed in a semester that is over (`failed`), and planned again.
    Again { failed: SemesterKey, planned: SemesterKey },
    /// Not passed in that semester, and not planned again.
    Failed(SemesterKey),
    Open,
}

impl RowState {
    /// The semester it is passed or planned in.
    pub fn semester(self) -> Option<SemesterKey> {
        match self {
            RowState::Passed(s) | RowState::Planned(s) => Some(s),
            RowState::Again { planned, .. } => Some(planned),
            RowState::Failed(_) | RowState::Open => None,
        }
    }
}

/// A module of the study that counts towards a choice, or fills a row's placeholder.
#[derive(Clone, Debug, PartialEq)]
pub struct Taken {
    pub id: String,
    /// The catalog's title, else the id.
    pub name: String,
    pub credits: Option<f64>,
    /// Where it is passed, else where it is planned (the first semester from the current one on).
    pub semester: SemesterKey,
    pub passed: bool,
}

/// A row of the plan in the study.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanLine {
    /// The caption of its plan and its `PlanEntry::ord`.
    pub caption: String,
    pub ord: i64,
    /// The catalog's title of its module, else the plan's name.
    pub name: String,
    pub module_id: Option<String>,
    /// What it is worth: the least of a range.
    pub credits: Option<f64>,
    /// As the plan prints them: „8", „10–24".
    pub credits_text: Option<String>,
    pub kind: Option<ModuleKind>,
    /// The Fachsemester the plan puts it into.
    pub first: u8,
    pub last: u8,
    /// The area of the progress it belongs to (`Study::progress`).
    pub area: Option<usize>,
    pub offer: Offer,
    pub state: RowState,
    /// The modules that fill its placeholders (`Planned::fills`), as the student chose them.
    pub fillers: Vec<Taken>,
    /// The choice it is one of (an index of `PlanView::choices`).
    pub choice: Option<usize>,
}

/// A list of modules a choice is made from: an area of the program's module tree, with the areas
/// below it.
#[derive(Clone, Debug, PartialEq)]
pub struct List {
    pub id: i64,
    /// The name the pickers show (`CatalogArea::name`).
    pub name: String,
    /// Its modules, each once, in the order the tree places them.
    pub modules: Vec<String>,
}

/// What the plan leaves to choose: its rows without a module of one name in one area.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// The rows' name („Anwendungsfach").
    pub name: String,
    pub area: Option<usize>,
    /// Its rows: indexes of `PlanView::lines`.
    pub lines: Vec<usize>,
    /// What it asks for: its rows together, the least and the most („10–24").
    pub least: f64,
    pub most: f64,
    /// As the plan prints it: „12", „10–24".
    pub credits_text: String,
    pub first: u8,
    pub last: u8,
    /// A module of the Fachübergreifendes Studium: its modules are the FÜS list, not the tree.
    pub fues: bool,
    /// What to choose from: the areas its rows point at, one of them where there are several
    /// („Anwendungsfach": Mathematik, Physik …). Empty where the tree says nothing.
    pub lists: Vec<List>,
    /// The sum the regulation prints over it and other choices, and their names („Summe Komplexe
    /// des Fachstudiums 44" with the two other Komplexe).
    pub together: Option<(f64, Vec<String>)>,
    /// What the student planned or passed of its lists, or put into its rows' placeholders.
    pub taken: Vec<Taken>,
    /// Its rows are all marked as done: the last semester one was.
    pub done: Option<SemesterKey>,
}

impl Choice {
    /// The credits of what is taken: passed and planned.
    pub fn taken_credits(&self) -> f64 {
        self.taken.iter().filter_map(|taken| taken.credits).sum()
    }

    pub fn passed_credits(&self) -> f64 {
        self.taken.iter().filter(|taken| taken.passed).filter_map(|taken| taken.credits).sum()
    }

    /// What it still asks for that is not taken.
    pub fn open(&self) -> f64 {
        if self.done.is_some() {
            return 0.0;
        }
        (self.least - self.taken_credits()).max(0.0)
    }

    /// Done, or what is taken reaches what it asks.
    pub fn complete(&self) -> bool {
        self.done.is_some() || (!self.taken.is_empty() && self.open() <= EPSILON)
    }

    /// Every module of its lists, each once.
    pub fn modules(&self) -> Vec<&str> {
        let mut seen = BTreeSet::new();
        self.lists.iter().flat_map(|list| list.modules.iter()).map(String::as_str).filter(|id| seen.insert(*id)).collect()
    }

    /// The list a module is on, the first that has it.
    pub fn list_of(&self, id: &str) -> Option<usize> {
        self.lists.iter().position(|list| list.modules.iter().any(|module| module == id))
    }
}

/// The plan as „Mein Studium" shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanView {
    /// Its rows, in the order of the document.
    pub lines: Vec<PlanLine>,
    /// Its choices, in the order of their first row.
    pub choices: Vec<Choice>,
    /// What the choices still ask for together that is not taken (a sum the regulation prints
    /// over several of them counted once).
    pub to_choose: f64,
}

/// A module of the study: passed, or planned and not failed.
struct Held {
    taken: Taken,
    /// The area of the progress it counts towards.
    area: Option<usize>,
}

/// The plan of `input` as „Mein Studium" shows it, with `study` (`study::study` of the same
/// input).
pub fn plan_view(input: &Input, study: &Study) -> PlanView {
    let rows = study::plan_rows(input.core, input.page);
    let doc = input.doc;
    let now = study.now;
    let catalog = |id: &str| input.rows.iter().find(|row| row.id == id);
    let area_of = |caption: &str, ord: i64| study.progress.iter().position(|p| p.area.rows.iter().any(|(c, o)| c == caption && *o == ord));
    let taken_of = |id: &str, semester: SemesterKey, passed: bool| {
        let row = catalog(id);
        Taken { id: id.to_string(), name: row.map_or_else(|| id.to_string(), |row| row.title.clone()), credits: row.and_then(|row| row.credits), semester, passed }
    };
    let program = input.program_id;

    let mut lines: Vec<PlanLine> = Vec::new();
    for row in &rows {
        let entry = row.entry;
        let caption = row.variant.full.clone();
        let module_id = entry.module_id.clone().filter(|id| url::is_module_id(id));
        let linked = module_id.as_deref().and_then(catalog);
        let (state, fillers) = match module_id.as_deref() {
            Some(id) => (module_state(doc.passed_in(id), doc.planned_in(id), now), Vec::new()),
            None => {
                let placed: Vec<_> = doc.placeholders.iter().filter(|p| p.program_id == program && p.ord == entry.ord && p.caption == caption).collect();
                let fillers: Vec<Taken> = placed
                    .iter()
                    .flat_map(|p| doc.fillers(p.pid))
                    .filter_map(|m| {
                        let passed = doc.passed_in(&m.module_id);
                        let failed = passed.is_none() && m.semester < now;
                        (!failed).then(|| taken_of(&m.module_id, passed.unwrap_or(m.semester), passed.is_some()))
                    })
                    .collect();
                let state = match doc.done_row(program, entry.ord) {
                    Some(done) => RowState::Passed(done.semester),
                    None if !fillers.is_empty() && fillers.iter().all(|filler| filler.passed) => {
                        fillers.iter().map(|filler| filler.semester).max().map_or(RowState::Open, RowState::Passed)
                    }
                    None if !fillers.is_empty() => fillers.iter().filter(|filler| !filler.passed).map(|filler| filler.semester).min().map_or(RowState::Open, RowState::Planned),
                    None => module_state(None, placed.iter().map(|p| p.semester).collect(), now),
                };
                (state, fillers)
            }
        };
        lines.push(PlanLine {
            name: linked.map(|row| row.title.clone()).unwrap_or_else(|| plan::shown_name(&entry.module_name).to_string()),
            credits: entry.credits.or(entry.min_credits).or(linked.and_then(|row| row.credits)),
            credits_text: plan::credits_of(entry),
            kind: entry.kind.as_ref().and_then(Code::known),
            first: row.first,
            last: row.last,
            area: area_of(&caption, entry.ord),
            offer: linked.map(Offer::of).unwrap_or_default(),
            state,
            fillers,
            choice: None,
            module_id,
            caption,
            ord: entry.ord,
        });
    }

    let choices = choices_of(input, study, &rows, &mut lines);
    let to_choose = to_choose(&rows, &choices);
    PlanView { lines, choices, to_choose }
}

/// Where a module stands: passed in `passed`, planned into `planned` (in order), `now` the current
/// semester. What was planned into a semester that is over and not passed there is failed.
fn module_state(passed: Option<SemesterKey>, planned: Vec<SemesterKey>, now: SemesterKey) -> RowState {
    if let Some(s) = passed {
        return RowState::Passed(s);
    }
    let failed = planned.iter().copied().filter(|s| *s < now).max();
    match (planned.into_iter().find(|s| *s >= now), failed) {
        (Some(planned), Some(failed)) => RowState::Again { failed, planned },
        (Some(planned), None) => RowState::Planned(planned),
        (None, Some(failed)) => RowState::Failed(failed),
        (None, None) => RowState::Open,
    }
}

/// The choices of the plan's rows (`lines`, which learn theirs), with what of their lists the
/// study holds.
fn choices_of(input: &Input, study: &Study, rows: &[PlanRow], lines: &mut [PlanLine]) -> Vec<Choice> {
    let doc = input.doc;
    let mut choices: Vec<Choice> = Vec::new();
    for (index, (row, line)) in rows.iter().zip(lines.iter_mut()).enumerate() {
        let entry = row.entry;
        if line.module_id.is_some() || plan::is_single_module(entry) {
            continue;
        }
        let fues = plan::is_fues(entry);
        let name = base_name(&line.name).to_string();
        let (least, most) = range_of(entry);
        let known = choices.iter().position(|choice| choice.area == line.area && choice.fues == fues && same_name(&choice.name, &name));
        let at = match known {
            Some(at) => at,
            None => {
                let lists = if fues { Vec::new() } else { lists_of(input, study, row, line.area) };
                choices.push(Choice {
                    name,
                    area: line.area,
                    lines: Vec::new(),
                    least: 0.0,
                    most: 0.0,
                    credits_text: String::new(),
                    first: row.first,
                    last: row.last,
                    fues,
                    lists,
                    together: together_of(row, rows, lines_names(rows)),
                    taken: Vec::new(),
                    done: None,
                });
                choices.len() - 1
            }
        };
        let Some(choice) = choices.get_mut(at) else { continue };
        choice.lines.push(index);
        choice.least += least;
        choice.most += most;
        choice.first = choice.first.min(row.first);
        choice.last = choice.last.max(row.last);
        line.choice = Some(at);
    }
    for choice in &mut choices {
        choice.credits_text = if choice.most > choice.least + EPSILON {
            format!("{}–{}", plan::number_text(choice.least), plan::number_text(choice.most))
        } else {
            plan::number_text(choice.least)
        };
        let done: Vec<Option<SemesterKey>> = choice.lines.iter().map(|i| doc.done_row(input.program_id, lines.get(*i).map_or(0, |line| line.ord)).map(|done| done.semester)).collect();
        if !done.is_empty() && done.iter().all(Option::is_some) {
            choice.done = done.into_iter().flatten().max();
        }
    }

    // What the study holds of them: the modules that fill a row's placeholders count there, the
    // others where a list has them, in the order of the plan.
    let held = held_of(study, rows);
    let mut counted: BTreeSet<String> = BTreeSet::new();
    for choice in &mut choices {
        for filler in choice.lines.iter().filter_map(|i| lines.get(*i)).flat_map(|line| line.fillers.iter()) {
            if counted.insert(filler.id.clone()) {
                choice.taken.push(filler.clone());
            }
        }
    }
    let fues_area = study.progress.iter().position(|p| p.area.kind == AreaKind::Fues);
    let mut order: Vec<usize> = (0..choices.len()).collect();
    order.sort_by_key(|at| choices.get(*at).map_or((u8::MAX, *at), |choice| (choice.first, *at)));
    for at in order {
        let Some(choice) = choices.get_mut(at) else { continue };
        let lists: BTreeSet<&str> = choice.lists.iter().flat_map(|list| list.modules.iter()).map(String::as_str).collect();
        for module in &held {
            if choice.taken_credits() + EPSILON >= choice.most.max(choice.least) || counted.contains(&module.taken.id) {
                continue;
            }
            let fits = if choice.fues { module.area.is_some() && module.area == fues_area } else { lists.contains(module.taken.id.as_str()) };
            if fits {
                counted.insert(module.taken.id.clone());
                choice.taken.push(module.taken.clone());
            }
        }
    }
    choices
}

/// The names of the rows, by their `ord` and plan.
fn lines_names<'a>(rows: &'a [PlanRow<'a>]) -> impl Fn(&str, i64) -> Option<String> + 'a {
    move |caption, ord| rows.iter().find(|row| row.variant.full == caption && row.entry.ord == ord).map(|row| plan::shown_name(&row.entry.module_name).to_string())
}

/// Two rows of the same name: „Anwendungsfach" and „Anwendungsfach".
fn same_name(a: &str, b: &str) -> bool {
    folia_search::fold(a.trim()) == folia_search::fold(b.trim())
}

/// A row's name without the number that counts it among rows of the same name: „Wahlpflichtmodul
/// 3" → „Wahlpflichtmodul" (Maschinenbau B.Sc. numbers its four).
fn base_name(name: &str) -> &str {
    let name = name.trim();
    match name.rsplit_once(' ') {
        Some((rest, last)) if !rest.trim().is_empty() && is_count(last) => rest.trim_end(),
        _ => name,
    }
}

/// „3", „3.", „III".
fn is_count(word: &str) -> bool {
    let word = word.trim_end_matches(['.', ')']);
    (!word.is_empty() && word.chars().all(|c| c.is_ascii_digit())) || ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"].contains(&word)
}

/// The least and the most a row asks for.
fn range_of(entry: &folia_model::rows_detail::PlanEntry) -> (f64, f64) {
    let least = entry.credits.or(entry.min_credits).unwrap_or(0.0);
    (least, entry.max_credits.unwrap_or(least).max(least))
}

/// The sum the regulation prints over `row` and the rows it is chosen with, and their names.
fn together_of(row: &PlanRow, rows: &[PlanRow], name_of: impl Fn(&str, i64) -> Option<String>) -> Option<(f64, Vec<String>)> {
    let total = study::choice_total(row)?;
    let caption = row.variant.full.as_str();
    let others: Vec<String> = total
        .entries
        .iter()
        .filter(|ord| **ord != row.entry.ord)
        .filter(|ord| rows.iter().any(|other| other.variant.full == caption && other.entry.ord == **ord && other.entry.max_credits.is_some()))
        .filter_map(|ord| name_of(caption, *ord))
        .collect();
    let fixed: f64 = row.variant.entries.iter().filter(|entry| total.entries.contains(&entry.ord) && entry.max_credits.is_none()).filter_map(|entry| entry.credits).sum();
    Some(((total.credits - fixed).max(0.0), others))
}

/// The lists of a choice of `row`: the areas of the tree its name points at, else the areas of
/// the progress's `area` that a student chooses in.
fn lists_of(input: &Input, study: &Study, row: &PlanRow, area: Option<usize>) -> Vec<List> {
    let tree = input.tree;
    let found = plan::areas_for_row(row.entry, &row.variant.full, tree, &row.variant.entries);
    let mut tops: Vec<&CatalogArea> = found.areas.iter().filter_map(|found| tree.iter().find(|area| area.id == found.id)).collect();
    if tops.is_empty() {
        // The tree's areas of the progress's area where a student chooses, the topmost of them.
        let claimed: Vec<&CatalogArea> = area.and_then(|area| study.progress.get(area)).map(|p| tree.iter().filter(|area| p.area.tree.contains(&area.id) && area.choice).collect()).unwrap_or_default();
        tops = claimed.iter().copied().filter(|area| !claimed.iter().any(|top| study::below(area, top))).collect();
    }
    tops.into_iter()
        .map(|top| {
            let mut seen = BTreeSet::new();
            let modules = input
                .placements
                .iter()
                .filter(|(_, at)| *at == top.id || tree.iter().any(|area| area.id == *at && study::below(area, top)))
                .map(|(module, _)| module.clone())
                .filter(|module| seen.insert(module.clone()))
                .collect();
            List { id: top.id, name: top.name.clone(), modules }
        })
        .filter(|list| !list.modules.is_empty())
        .collect()
}

/// The modules of the study that are passed, or planned and not failed, each once: what is passed
/// first, then by semester. Not the ones the plan names in a row of their own: they count there.
fn held_of(study: &Study, rows: &[PlanRow]) -> Vec<Held> {
    let named: BTreeSet<&str> = rows.iter().filter_map(|row| row.entry.module_id.as_deref()).collect();
    let mut held: Vec<Held> = Vec::new();
    for semester in &study.semesters {
        for item in semester.items.iter().filter(|item| item.passed || !item.failed) {
            let Some(id) = item.module_id() else { continue };
            if named.contains(id) {
                continue;
            }
            let taken = Taken { id: id.to_string(), name: item.name.clone(), credits: item.credits, semester: semester.key, passed: item.passed };
            match held.iter_mut().find(|known| known.taken.id == id) {
                Some(known) if item.passed && !known.taken.passed => known.taken = taken,
                Some(_) => {}
                None => held.push(Held { area: item.area, taken }),
            }
        }
    }
    held.sort_by_key(|module| (!module.taken.passed, module.taken.semester));
    held
}

/// What the choices still ask for together: a sum the regulation prints over several counted
/// once, less what is taken of all of them.
fn to_choose(rows: &[PlanRow], choices: &[Choice]) -> f64 {
    let mut sum = 0.0;
    let mut totals: Vec<(&str, i64)> = Vec::new();
    for choice in choices {
        let tied = choice.lines.iter().filter_map(|i| rows.get(*i)).find_map(|row| study::choice_total(row).map(|total| (row.variant.full.as_str(), total)));
        match tied {
            Some((caption, total)) => {
                if totals.contains(&(caption, total.ord)) {
                    continue;
                }
                totals.push((caption, total.ord));
                let members: Vec<&Choice> = choices
                    .iter()
                    .filter(|other| other.lines.iter().filter_map(|i| rows.get(*i)).any(|row| row.variant.full == caption && total.entries.contains(&row.entry.ord)))
                    .collect();
                let asked = choice.together.as_ref().map_or(choice.least, |(credits, _)| *credits);
                if members.iter().all(|member| member.done.is_some()) {
                    continue;
                }
                let taken: f64 = members.iter().map(|member| member.taken_credits().min(member.most.max(member.least))).sum();
                sum += (asked - taken).max(0.0);
            }
            None => sum += choice.open(),
        }
    }
    sum
}

/// The catalog's rows a view of the choices asks for: every module of their lists.
pub fn list_modules(view: &PlanView) -> Vec<String> {
    let mut ids: Vec<String> = view.choices.iter().flat_map(|choice| choice.modules().into_iter().map(str::to_string)).collect();
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    /// Where a module stands, seen from WiSe 2026/27.
    #[test]
    fn a_module_s_stand() {
        let now = key("2026W");
        assert_eq!(module_state(Some(key("2025W")), vec![key("2025W")], now), RowState::Passed(key("2025W")));
        assert_eq!(module_state(None, vec![key("2027S")], now), RowState::Planned(key("2027S")));
        assert_eq!(module_state(None, vec![key("2026W")], now), RowState::Planned(now), "the current semester is not over");
        assert_eq!(module_state(None, vec![key("2025W"), key("2027S")], now), RowState::Again { failed: key("2025W"), planned: key("2027S") });
        assert_eq!(module_state(None, vec![key("2025W"), key("2026S")], now), RowState::Failed(key("2026S")), "the last semester it was not passed in");
        assert_eq!(module_state(None, Vec::new(), now), RowState::Open);
        assert_eq!(RowState::Again { failed: key("2025W"), planned: key("2027S") }.semester(), Some(key("2027S")));
        assert_eq!(RowState::Failed(key("2025W")).semester(), None);
    }

    /// Rows of one name count together, their number aside.
    #[test]
    fn the_name_of_a_choice() {
        assert_eq!(base_name("Wahlpflichtmodul 3"), "Wahlpflichtmodul");
        assert_eq!(base_name("Wahlpflichtmodul 3."), "Wahlpflichtmodul");
        assert_eq!(base_name("Projekt II"), "Projekt");
        assert_eq!(base_name("Anwendungsfach"), "Anwendungsfach");
        assert_eq!(base_name("3"), "3", "a name that is only a number stays");
        assert_eq!(base_name("Modul aus dem Bereich Praktische Mathematik"), "Modul aus dem Bereich Praktische Mathematik");
        assert!(same_name("Anwendungsfach", " anwendungsfach "));
    }
}
