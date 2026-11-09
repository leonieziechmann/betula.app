//! The workbench of „Mein Studium" (folia/design/hinzufuegen/konzepte.html, third round; owner,
//! 2026-11-03: „Passt so, bau das jetzt in Folia"): planning one semester by the areas of the
//! overview and their subgroups. An area's subgroups are its Pflicht — the rows of the plan that
//! name a module or are one module of their own — and each of its choices (`choices::Choice`): the
//! Informatik's Pflicht and „Proseminar oder Praktikum", the Nebenfach's „Anwendungsfach" and
//! „Praktische Mathematik". An area of one subgroup has none to show: it is its Pflicht or its one
//! choice.
//!
//! What the plan puts into the semester's Fachsemester („Laut Plan im 3. FS", `Due`): its Pflicht
//! and its choices, what is open from the Fachsemester before it, and what the next one could take
//! now: what the semester offers and has in place what it requires (`Due::ahead`). A module
//! planned from a choice counts for it (`plan_for`): where a placeholder of the choice's rows
//! stands in the semester and nothing fills it, the module fills it (`study::add_for`), else it
//! remembers the choice's first row (`Planned::counts`); the plan names a module of the Pflicht
//! itself. Nothing is stored for what the plan only expects: it is worked out anew.

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;

use folia_model::labels::{Code, ModuleKind};

use crate::choices::{PlanView, RowState};
use crate::study::{self, AreaKind, Input, Pick, Study, EPSILON};
use crate::studyplan::{PlanDoc, RowRef};

/// What a subgroup is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupKind {
    /// The Pflicht of the area: its rows that are no choice.
    Pflicht,
    /// A choice of the plan: an index of `PlanView::choices`.
    Choice(usize),
}

/// A subgroup of an area.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub kind: GroupKind,
    /// The area of the progress (`Study::progress`).
    pub area: usize,
    /// Its rows: indexes of `PlanView::lines`, in the order of the plan.
    pub lines: Vec<usize>,
    /// The Fachsemester its rows lie in.
    pub first: u8,
    pub last: u8,
    /// Every row passed, planned or done; a choice whose taken modules reach what it asks.
    pub complete: bool,
    /// The Pflicht's rows that stand nowhere yet: neither passed nor planned from the current
    /// semester on.
    pub open: usize,
}

impl Group {
    /// The choice, where it is one.
    pub fn choice(&self) -> Option<usize> {
        match self.kind {
            GroupKind::Choice(at) => Some(at),
            GroupKind::Pflicht => None,
        }
    }

    /// Its span includes Fachsemester `fs`.
    pub fn spans(&self, fs: u8) -> bool {
        self.first <= fs && fs <= self.last
    }
}

/// An area of the progress with its subgroups.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub area: usize,
    /// Indexes of `Bench::groups`: the Pflicht first, then the choices in the order of the plan.
    pub groups: Vec<usize>,
}

/// The areas and their subgroups.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bench {
    pub groups: Vec<Group>,
    pub areas: Vec<Area>,
}

/// A row that stands nowhere: neither passed nor planned from the current semester on.
fn stands_nowhere(state: RowState) -> bool {
    matches!(state, RowState::Open | RowState::Failed(_))
}

impl Bench {
    /// The areas of `study` with their subgroups, from the plan's view of it (`choices::plan_view`
    /// of the same study).
    pub fn of(study: &Study, view: &PlanView) -> Self {
        let mut groups: Vec<Group> = Vec::new();
        let mut areas: Vec<Area> = Vec::new();
        for area in 0..study.progress.len() {
            let mut own = Vec::new();
            let pflicht: Vec<usize> = view.lines.iter().enumerate().filter(|(_, line)| line.area == Some(area) && line.choice.is_none()).map(|(at, _)| at).collect();
            if !pflicht.is_empty() {
                let rows = pflicht.iter().filter_map(|at| view.lines.get(*at));
                let (first, last) = rows.clone().fold((u8::MAX, 0), |(first, last), line| (first.min(line.first), last.max(line.last)));
                let open = rows.filter(|line| stands_nowhere(line.state)).count();
                own.push(groups.len());
                groups.push(Group { kind: GroupKind::Pflicht, area, lines: pflicht, first, last, complete: open == 0, open });
            }
            // The choices by the Fachsemester they begin in, then in the order of the plan: an
            // order that what is planned never changes.
            let mut choices: Vec<(usize, &crate::choices::Choice)> = view.choices.iter().enumerate().filter(|(_, choice)| choice.area == Some(area)).collect();
            choices.sort_by_key(|(at, choice)| (choice.first, choice.last, *at));
            for (at, choice) in choices {
                own.push(groups.len());
                groups.push(Group { kind: GroupKind::Choice(at), area, lines: choice.lines.clone(), first: choice.first, last: choice.last, complete: choice.complete(), open: 0 });
            }
            if !own.is_empty() {
                areas.push(Area { area, groups: own });
            }
        }
        Bench { groups, areas }
    }

    /// The area of the progress `area`, as the workbench lists it.
    pub fn area(&self, area: usize) -> Option<&Area> {
        self.areas.iter().find(|known| known.area == area)
    }

    /// The subgroup of `kind` in `area`.
    pub fn find(&self, area: usize, kind: GroupKind) -> Option<usize> {
        self.groups.iter().position(|group| group.area == area && group.kind == kind)
    }

    /// The subgroups of `area` to show (the workbench's tree, the Gesamtplan's subsections): a
    /// choice whose modules other choices of the area list all of is none of them (owner,
    /// 2026-11-04: „Auch hier braucht es keinen kombinierten bereich aus mehreren wp dingen, weil
    /// die ja von den bereichen selbst abgedeckt werden"): the Fachstudium's „Wahlpflicht: Komplex
    /// … / … / …" draws from the three Komplexe, each of which stands there.
    pub fn shown(&self, view: &PlanView, area: usize) -> Vec<usize> {
        let Some(own) = self.area(area) else { return Vec::new() };
        let lists_of = |at: usize| -> Option<BTreeSet<&str>> {
            let choice = view.choices.get(self.groups.get(at)?.choice()?)?;
            (!choice.fues && !choice.lists.is_empty()).then(|| choice.lists.iter().flat_map(|list| list.modules.iter().map(String::as_str)).collect())
        };
        own.groups
            .iter()
            .copied()
            .filter(|at| {
                let Some(mine) = lists_of(*at).filter(|mine| !mine.is_empty()) else { return true };
                let others: BTreeSet<&str> = own.groups.iter().filter(|other| *other != at).filter_map(|other| lists_of(*other)).filter(|theirs| theirs.len() < mine.len()).flatten().collect();
                !mine.is_subset(&others)
            })
            .collect()
    }

    /// The subgroup of a choice.
    pub fn of_choice(&self, choice: usize) -> Option<usize> {
        self.groups.iter().position(|group| group.kind == GroupKind::Choice(choice))
    }

    /// The subgroup an area opens with for Fachsemester `fs`: the one that is due then — a choice
    /// not complete whose rows lie in it or before it, a Pflicht with a row of it or of one before
    /// it that stands nowhere —, else the first not complete, else the first.
    pub fn opens_with(&self, view: &PlanView, area: usize, fs: Option<u8>) -> Option<usize> {
        let own = &self.area(area)?.groups;
        let due = |at: &usize| {
            let Some(group) = self.groups.get(*at) else { return false };
            let Some(fs) = fs else { return false };
            match group.kind {
                GroupKind::Choice(_) => !group.complete && group.first <= fs,
                GroupKind::Pflicht => group.lines.iter().filter_map(|i| view.lines.get(*i)).any(|line| line.first <= fs && stands_nowhere(line.state)),
            }
        };
        own.iter().copied().find(|at| due(at)).or_else(|| own.iter().copied().find(|at| self.groups.get(*at).is_some_and(|group| !group.complete))).or_else(|| own.first().copied())
    }

    /// The subgroup a module counts for where nothing says otherwise: the Pflicht whose row names
    /// it; the choice it is planned for (`counts`, a choice of those rows) or the Pflicht of the row
    /// it is planned for; of the choices whose lists hold it one not complete, the most specific
    /// (the shortest lists), the first; the FÜS where its list (`fues`) holds it.
    pub fn group_of(&self, view: &PlanView, id: &str, counts: Option<&RowRef>, program_id: &str, fues: &[String]) -> Option<usize> {
        if let Some(at) = view.lines.iter().position(|line| line.module_id.as_deref() == Some(id)) {
            let line = view.lines.get(at)?;
            return match line.choice {
                Some(choice) => self.of_choice(choice),
                None => self.groups.iter().position(|group| group.kind == GroupKind::Pflicht && group.lines.contains(&at)),
            };
        }
        if let Some(row) = counts {
            if let Some(at) = view.lines.iter().position(|line| row.is(program_id, &line.caption, line.ord)) {
                return match view.lines.get(at).and_then(|line| line.choice) {
                    Some(choice) => self.of_choice(choice),
                    None => self.groups.iter().position(|group| group.kind == GroupKind::Pflicht && group.lines.contains(&at)),
                };
            }
        }
        // As the study counts it (`choices::plan_view`): a choice that still asks for credits first,
        // then the one with the shortest lists, then the first in the plan.
        let mut holding: Vec<(bool, usize, u8, usize)> =
            view.choices.iter().enumerate().filter(|(_, choice)| !choice.fues && choice.list_of(id).is_some()).map(|(at, choice)| (choice.complete(), choice.modules().len(), choice.first, at)).collect();
        holding.sort_unstable();
        if let Some(&(.., choice)) = holding.first() {
            return self.of_choice(choice);
        }
        let in_fues = fues.iter().any(|known| known == id);
        view.choices.iter().position(|choice| choice.fues && in_fues).and_then(|choice| self.of_choice(choice))
    }
}

/// The subgroup a module of a semester counts for, as the plan's view has it: the Pflicht whose
/// row names it, the Pflicht the student let it count for, or the choice whose taken modules hold
/// it in that semester.
pub fn counted_in(bench: &Bench, view: &PlanView, id: &str, s: SemesterKey) -> Option<usize> {
    if let Some(at) = view.lines.iter().position(|line| line.module_id.as_deref() == Some(id) && line.choice.is_none()) {
        return bench.groups.iter().position(|group| group.kind == GroupKind::Pflicht && group.lines.contains(&at));
    }
    // Counted for an area's Pflicht by the student (`PlanView::pflicht`).
    if let Some((_, at)) = view.pflicht.iter().find(|(taken, _)| taken.id == id && taken.semester == s) {
        return bench.groups.iter().position(|group| group.kind == GroupKind::Pflicht && group.lines.contains(at));
    }
    let choice = view.choices.iter().position(|choice| choice.taken.iter().any(|taken| taken.id == id && taken.semester == s))?;
    bench.of_choice(choice)
}

/// What the plan puts into a Fachsemester, as „Laut Plan im 3. FS" lists it: rows and choices by
/// their indexes in `PlanView`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Due {
    /// The Pflicht's rows of the Fachsemester, in the order of the plan.
    pub pflicht: Vec<usize>,
    /// The choices whose rows lie in it.
    pub choices: Vec<usize>,
    /// What is open from the Fachsemester before it: the Pflicht's rows that stand nowhere, the
    /// choices not complete.
    pub earlier: Vec<usize>,
    pub earlier_choices: Vec<usize>,
    /// What the next Fachsemester could take now: its Pflicht's rows that stand nowhere, its
    /// choices not complete that begin there.
    pub next: Vec<usize>,
    pub next_choices: Vec<usize>,
}

impl Due {
    /// What the plan puts into Fachsemester `fs`; without one (a semester of leave, one before the
    /// Studienbeginn) only what is open from before it, the current semester's Fachsemester `now`.
    pub fn of(view: &PlanView, fs: Option<u8>, now: Option<u8>) -> Self {
        let mut due = Due::default();
        let before = fs.or(now).unwrap_or(1);
        for (at, line) in view.lines.iter().enumerate().filter(|(_, line)| line.choice.is_none()) {
            match fs {
                Some(fs) if line.first <= fs && fs <= line.last => due.pflicht.push(at),
                Some(fs) if line.first == fs + 1 && stands_nowhere(line.state) => due.next.push(at),
                _ if line.last < before && stands_nowhere(line.state) => due.earlier.push(at),
                _ => {}
            }
        }
        for (at, choice) in view.choices.iter().enumerate() {
            match fs {
                Some(fs) if choice.first <= fs && fs <= choice.last => due.choices.push(at),
                Some(fs) if choice.first == fs + 1 && !choice.complete() => due.next_choices.push(at),
                _ if choice.last < before && !choice.complete() => due.earlier_choices.push(at),
                _ => {}
            }
        }
        due
    }

    /// What „Aus dem 4. FS vorziehen" suggests of `next` and `next_choices` for semester `s`
    /// (owner, 2026-11-05: „das ergibt ja nur sinn für module, die in dem ausgewählten semester
    /// auch angeboten werden und wofür die voraussetzungen erfüllt sind"), `takes` saying of a
    /// module that `s` can take it: the Pflicht's rows whose module it takes (a row without one
    /// says nothing either way and stays); the choices that take one of their modules in `s`
    /// already, or have one in their lists it takes (`fues`, the FÜS list, for the FÜS). A choice
    /// without lists stays: nothing says it cannot be taken. What is not suggested stays where the
    /// plan puts it and in its subgroup's list, from where it can be planned into `s` all the same.
    pub fn ahead(&self, view: &PlanView, bench: &Bench, s: SemesterKey, fues: &[String], takes: impl Fn(&str) -> bool) -> (Vec<usize>, Vec<usize>) {
        let rows = self.next.iter().copied().filter(|at| view.lines.get(*at).is_some_and(|line| line.module_id.as_deref().is_none_or(&takes))).collect();
        let choices = self
            .next_choices
            .iter()
            .copied()
            .filter(|at| {
                let Some(choice) = view.choices.get(*at) else { return false };
                if choice.taken.iter().any(|taken| taken.semester == s) {
                    return true;
                }
                let Some(group) = bench.of_choice(*at).and_then(|group| bench.groups.get(group)) else { return true };
                let modules = candidates(view, group, fues);
                modules.is_empty() || modules.iter().any(|(id, _)| takes(id))
            })
            .collect();
        (rows, choices)
    }

    /// The modules „Plan übernehmen" plans into semester `s`: the Pflicht's rows of the
    /// Fachsemester with a module that stand nowhere yet and that `s` offers.
    pub fn takeover(&self, view: &PlanView, s: SemesterKey) -> Vec<String> {
        self.pflicht
            .iter()
            .filter_map(|at| view.lines.get(*at))
            .filter(|line| line.state == RowState::Open && line.offer.offered(s))
            .filter_map(|line| line.module_id.clone())
            .collect()
    }
}

/// The modules of a subgroup to choose from: the modules its rows name (the Pflicht), the modules
/// of a choice's lists, each with the list it is on (the „Fach" of the Anwendungsfach), or the
/// FÜS list (`fues`) for the FÜS.
pub fn candidates(view: &PlanView, group: &Group, fues: &[String]) -> Vec<(String, Option<usize>)> {
    let mut seen = BTreeSet::new();
    match group.kind {
        GroupKind::Pflicht => group.lines.iter().filter_map(|at| view.lines.get(*at)?.module_id.clone()).filter(|id| seen.insert(id.clone())).map(|id| (id, None)).collect(),
        GroupKind::Choice(at) => {
            let Some(choice) = view.choices.get(at) else { return Vec::new() };
            if choice.fues {
                return fues.iter().filter(|id| seen.insert((*id).clone())).map(|id| (id.clone(), None)).collect();
            }
            choice
                .lists
                .iter()
                .enumerate()
                .flat_map(|(list, known)| known.modules.iter().map(move |id| (id.clone(), Some(list))))
                .filter(|(id, _)| seen.insert(id.clone()))
                .collect()
        }
    }
}

/// The first row of a choice: what a module planned for it remembers (`Planned::counts`).
pub fn row_of(view: &PlanView, choice: usize, program_id: &str) -> Option<RowRef> {
    let line = view.choices.get(choice)?.lines.first().and_then(|at| view.lines.get(*at))?;
    Some(RowRef::new(program_id, &line.caption, line.ord))
}

/// Its rows, each its plan's caption and `ord`: what a placeholder of the subgroup is one of.
pub fn rows_of(view: &PlanView, group: &Group) -> Vec<(String, i64)> {
    group.lines.iter().filter_map(|at| view.lines.get(*at)).map(|line| (line.caption.clone(), line.ord)).collect()
}

/// Plans module `id` into semester `s` from subgroup `group` („+ Einplanen"); how many it added.
/// From a choice, a placeholder of its rows standing in `s` that nothing fills is filled by it
/// (`study::add_for`; stored placeholders stay valid: owner, 2026-11-03, „Gespeicherte Platzhalter
/// bleiben gültig"), else the module remembers the choice's first row (`Planned::counts`), so that
/// it counts there and nowhere else. From the Pflicht it comes from the plan (`Pick::Module`
/// `from_plan`), whose row names it. Without a subgroup (the Merkliste, a module the plan does not
/// name) it counts where the tree places it.
pub fn plan_for(doc: &mut PlanDoc, input: &Input, view: &PlanView, s: SemesterKey, id: &str, group: Option<&Group>, at: u64) -> usize {
    match group.map(|group| (group, group.kind)) {
        Some((group, GroupKind::Choice(choice))) => {
            let rows = rows_of(view, group);
            let added = study::add_for(doc, input, s, id, &rows, at);
            let filled = doc.modules.iter().any(|m| m.semester == s && m.module_id == id && m.fills.is_some());
            if added > 0 && !filled {
                doc.set_counts(s, id, row_of(view, choice, input.program_id));
            }
            added
        }
        Some((_, GroupKind::Pflicht)) => study::add(doc, input, s, &[Pick::Module { id: id.to_string(), from_plan: true }], at),
        None => study::add(doc, input, s, &[Pick::Module { id: id.to_string(), from_plan: false }], at),
    }
}

/// „Zählt für": lets module `id` of semester `s` count for the subgroup `group` (its first row:
/// a choice's, or a row of an area's Pflicht), whatever its lists hold (owner, 2026-11-08: „jedes
/// modul zu jedem möglichen bereich zuzuordnen"); `None` leaves it to the study again
/// (`choices::plan_view`). A placeholder it filled it fills no more.
pub fn count_for(doc: &mut PlanDoc, view: &PlanView, s: SemesterKey, id: &str, group: Option<&Group>, program_id: &str) {
    let row = group.and_then(|group| row_of_group(view, group, program_id));
    doc.set_fills(s, id, None);
    doc.set_counts(s, id, row);
}

/// The first row of a subgroup: what a module counted for it remembers (`Planned::counts`).
pub fn row_of_group(view: &PlanView, group: &Group, program_id: &str) -> Option<RowRef> {
    match group.kind {
        GroupKind::Choice(choice) => row_of(view, choice, program_id),
        GroupKind::Pflicht => group.lines.first().and_then(|at| view.lines.get(*at)).map(|line| RowRef::new(program_id, &line.caption, line.ord)),
    }
}

/// Whether module `id` belongs to subgroup `group` by what the plan and the tree say, as the study
/// tells an exception (`study::Item::exception`): a choice its lists (the FÜS list for the FÜS; a
/// choice without lists takes anything), an area's Pflicht what the area's module tree holds (the
/// thesis's a thesis, the FÜS's the FÜS list, an area the tree says nothing of anything).
pub fn fits(input: &Input, study: &Study, view: &PlanView, group: &Group, id: &str) -> bool {
    let is_fues = |id: &str| input.fues.iter().any(|known| known == id) || input.rows.iter().any(|row| row.id == id && row.is_fues);
    match group.kind {
        GroupKind::Choice(at) => match view.choices.get(at) {
            Some(choice) if choice.fues => is_fues(id),
            Some(choice) => choice.lists.is_empty() || choice.list_of(id).is_some(),
            None => true,
        },
        GroupKind::Pflicht => {
            let Some(area) = study.progress.get(group.area).map(|progress| &progress.area) else { return true };
            match area.kind {
                AreaKind::Fues => is_fues(id),
                AreaKind::Thesis => input.rows.iter().any(|row| row.id == id && row.kind.as_ref().and_then(Code::known) == Some(ModuleKind::Thesis)),
                AreaKind::Plan | AreaKind::Other => area.tree.is_empty() || input.placements.iter().any(|(module, at)| module == id && area.tree.contains(at)),
            }
        }
    }
}

/// What a subgroup has: the credits of its Pflicht's rows passed or planned, of the modules a
/// choice takes; and what it asks.
pub fn figures(view: &PlanView, group: &Group) -> (f64, f64) {
    match group.kind {
        GroupKind::Pflicht => {
            let rows = group.lines.iter().filter_map(|at| view.lines.get(*at));
            let asked: f64 = rows.clone().filter_map(|line| line.credits).sum();
            let have: f64 = rows.filter(|line| !stands_nowhere(line.state)).filter_map(|line| line.credits).sum();
            (have, asked)
        }
        GroupKind::Choice(at) => view.choices.get(at).map_or((0.0, 0.0), |choice| (choice.taken_credits(), choice.least)),
    }
}

/// Whether what a group has reaches what it asks, for a figure „6/12" against „✓".
pub fn reaches(have: f64, asked: f64) -> bool {
    have + EPSILON >= asked
}

/// The name of a subgroup where a list has little room: „Modul aus dem Bereich Praktische
/// Mathematik" → „Praktische Mathematik", „Komplex Praktische Informatik" → „Praktische
/// Informatik", „Wahlpflicht: Komplex A / Komplex B" → „Wahlpflicht". Its head says it whole.
pub fn short_name(name: &str) -> String {
    let name = name.trim();
    let mut rest = name;
    for prefix in ["Modul aus dem Bereich ", "Module aus dem Bereich ", "Modul aus der ", "Module aus der ", "Modul aus ", "Module aus "] {
        if rest.len() > prefix.len() && rest.is_char_boundary(prefix.len()) && rest[..prefix.len()].eq_ignore_ascii_case(prefix) {
            rest = rest[prefix.len()..].trim_start();
            break;
        }
    }
    let rest = study::area_name(rest);
    match rest.split_once(':') {
        Some((head, tail)) if tail.contains(" / ") && head.trim().chars().count() >= 3 => head.trim().to_string(),
        _ => rest.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subgroup_s_short_name() {
        assert_eq!(short_name("Modul aus dem Bereich Praktische Mathematik"), "Praktische Mathematik");
        assert_eq!(short_name("Komplex Praktische Informatik"), "Praktische Informatik");
        assert_eq!(short_name("Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik"), "Wahlpflicht");
        assert_eq!(short_name("Anwendungsfach"), "Anwendungsfach");
        assert_eq!(short_name("Seminar oder Praktikum"), "Seminar oder Praktikum");
        assert_eq!(short_name("Wahlpflichtmodule Praktische Mathematik"), "Praktische Mathematik");
    }
}
