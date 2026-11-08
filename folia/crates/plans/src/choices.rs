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
//! (`study::study`). A module counts towards one choice at most: the one whose row it was planned
//! for (`Planned::counts`, the workbench of folia/design/hinzufuegen/konzepte.html), else the
//! first, in the order of the plan, that lists it and has room left.
//!
//! Choices the regulation sums over are one requirement, a `Group` (folia/design/wahlpflicht,
//! second round): „Summe Komplexe des Fachstudiums 44" over the three Komplexe of „10–24", with the
//! choice whose lists are theirs („Wahlpflicht: Komplex … / … / …", 6 LP in the 4. FS of Informatik
//! B.Sc.) — 50 LP of the three, each 10 to 24, and the one over 24 may go to 30: the extra's
//! credits count for one member alone (owner, 2026-11-04: „nur eins davon darf 30 haben"). What the
//! study holds of a group falls to the member whose list has it, whichever of the group's choices
//! counted it.

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Code, ModuleKind};
use folia_routes::url;

use crate::areas::CatalogArea;
use crate::plan;
use crate::study::{self, AreaKind, Input, Offer, PlanRow, Study, EPSILON};
use crate::studyplan::{PlanDoc, RowRef};

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
    /// It counts here because the student said so, and this choice does not list it
    /// (`study::Item::exception`).
    pub exception: bool,
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
    /// The choices the regulation sums over, each one requirement (`Group`).
    pub groups: Vec<Group>,
    /// The modules of the study on a choice's list that count for none, its room being full (a
    /// second Proseminar): beyond what is asked, said where they are listed.
    pub spare: Vec<Taken>,
    /// The modules the student let count for a row of the plan that is no choice (`Planned::counts`
    /// at a Pflicht row: „Zählt für" an area's Pflichtmodule), with that row (an index of `lines`):
    /// they count for its area and for no choice.
    pub pflicht: Vec<(Taken, usize)>,
    /// What the choices still ask for together that is not taken (a sum the regulation prints
    /// over several of them counted once, a group as `Group::open`).
    pub to_choose: f64,
}

/// What a choice or a group holds against what it asks: its bar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stand {
    pub passed: f64,
    /// Planned and not passed.
    pub planned: f64,
    pub least: f64,
    /// The most it takes: the room of a group's member is the more for the one that keeps the
    /// extras' credits (`Room::Kept`).
    pub most: f64,
}

impl Stand {
    pub fn taken(&self) -> f64 {
        self.passed + self.planned
    }

    /// What is held of `taken`, as the bar splits it.
    fn of<'a>(taken: impl IntoIterator<Item = &'a Taken>, least: f64, most: f64) -> Self {
        let mut stand = Stand { least, most, ..Stand::default() };
        for taken in taken {
            match taken.passed {
                true => stand.passed += taken.credits.unwrap_or(0.0),
                false => stand.planned += taken.credits.unwrap_or(0.0),
            }
        }
        stand
    }
}

/// The room above a group member's most that the group's extras give one member: none where the
/// group has no extras; open to each while no member is over its most; kept by the member over it
/// with the most; gone for the others then; and over, for a member past its most besides the one
/// that keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    None,
    Open,
    Kept,
    Gone,
    Over,
}

/// A member of a group as the Wahlpflicht shows it: what of the study falls on its lists, and the
/// room above its most.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    /// An index of `PlanView::choices`.
    pub choice: usize,
    pub taken: Vec<Taken>,
    pub stand: Stand,
    pub room: Room,
}

/// Choices the regulation prints one sum over (`together`), each with lists of its own, and the
/// choices whose lists are all of theirs and nothing else (`extras`): one requirement.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Indexes of `PlanView::choices`, in the plan's order.
    pub members: Vec<usize>,
    pub extras: Vec<usize>,
    /// What the sum asks of the members („44"), and the sum's label without „Summe"
    /// („Komplexe des Fachstudiums").
    pub together: f64,
    pub label: String,
}

impl Group {
    /// Every choice of it.
    pub fn choices(&self) -> impl Iterator<Item = usize> + '_ {
        self.members.iter().chain(self.extras.iter()).copied()
    }

    /// What the extras ask, the least and the most: the room one member has beyond its most.
    pub fn extra(&self, choices: &[Choice]) -> (f64, f64) {
        self.extras.iter().filter_map(|at| choices.get(*at)).fold((0.0, 0.0), |(least, most), choice| (least + choice.least, most + choice.most.max(choice.least)))
    }

    /// What it asks: the sum and the extras.
    pub fn target(&self, choices: &[Choice]) -> f64 {
        self.together + self.extra(choices).0
    }

    /// Its rows are all marked as done.
    pub fn done(&self, choices: &[Choice]) -> bool {
        self.choices().all(|at| choices.get(at).is_some_and(|choice| choice.done.is_some()))
    }

    /// Its members: what the group's choices count, and what counts for none (`PlanView::spare`),
    /// falls to the member whose list has it; the member over its most with the most keeps the
    /// extras' room.
    pub fn members(&self, view: &PlanView) -> Vec<Member> {
        let choices = &view.choices;
        let room = self.extra(choices).1;
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let held: Vec<&Taken> = self.choices().filter_map(|at| choices.get(at)).flat_map(|choice| choice.taken.iter()).chain(view.spare.iter()).filter(|taken| seen.insert(taken.id.as_str())).collect();
        let mut members: Vec<Member> = self
            .members
            .iter()
            .filter_map(|at| choices.get(*at).map(|choice| (*at, choice)))
            .map(|(at, choice)| {
                let taken: Vec<Taken> = held.iter().filter(|taken| choice.list_of(&taken.id).is_some()).map(|taken| (*taken).clone()).collect();
                let stand = Stand::of(&taken, choice.least, choice.most.max(choice.least));
                Member { choice: at, taken, stand, room: if room > EPSILON { Room::Open } else { Room::None } }
            })
            .collect();
        if room <= EPSILON {
            return members;
        }
        let over: Vec<usize> = members.iter().enumerate().filter(|(_, member)| member.stand.taken() > member.stand.most + EPSILON).map(|(at, _)| at).collect();
        // The one with the most keeps it; of two alike, the one over its most in an earlier
        // semester, so that what is added later does not take it away.
        let crossed = |member: &Member| {
            let mut taken: Vec<&Taken> = member.taken.iter().collect();
            taken.sort_by_key(|taken| taken.semester);
            let mut sum = 0.0;
            taken.into_iter().find(|taken| {
                sum += taken.credits.unwrap_or(0.0);
                sum > member.stand.most + EPSILON
            }).map(|taken| taken.semester)
        };
        let keeps = over
            .iter()
            .filter_map(|at| members.get(*at).map(|member| (*at, member)))
            .max_by(|(a, a_), (b, b_)| a_.stand.taken().total_cmp(&b_.stand.taken()).then_with(|| crossed(b_).cmp(&crossed(a_))).then(b.cmp(a)))
            .map(|(at, _)| at);
        for (at, member) in members.iter_mut().enumerate() {
            member.room = match keeps {
                None => Room::Open,
                Some(kept) if kept == at => Room::Kept,
                Some(_) if over.contains(&at) => Room::Over,
                Some(_) => Room::Gone,
            };
            if member.room == Room::Kept {
                member.stand.most += room;
            }
        }
        members
    }

    /// Its stand as a whole: what its members hold, against what it asks.
    pub fn stand(&self, view: &PlanView) -> Stand {
        let target = self.target(&view.choices);
        if self.done(&view.choices) {
            return Stand { passed: target, planned: 0.0, least: target, most: target };
        }
        let members = self.members(view);
        Stand { passed: members.iter().map(|member| member.stand.passed).sum(), planned: members.iter().map(|member| member.stand.planned).sum(), least: target, most: target }
    }

    /// What it still asks: of what it asks, what its members hold up to their most, and at least
    /// what each still lacks of its least.
    pub fn open(&self, view: &PlanView) -> f64 {
        if self.done(&view.choices) {
            return 0.0;
        }
        let members = self.members(view);
        let held: f64 = members.iter().map(|member| member.stand.taken().min(member.stand.most)).sum();
        let lacking: f64 = members.iter().map(|member| (member.stand.least - member.stand.taken()).max(0.0)).sum();
        (self.target(&view.choices) - held).max(lacking).max(0.0)
    }
}

impl PlanView {
    /// The group a choice is part of.
    pub fn group_of(&self, choice: usize) -> Option<&Group> {
        self.groups.iter().find(|group| group.choices().any(|at| at == choice))
    }

    /// The stand of a choice that is part of no group: what it counts, and what of the study on
    /// its lists counts for none where it is the first choice in the plan to list it.
    pub fn stand_of(&self, at: usize) -> Stand {
        let Some(choice) = self.choices.get(at) else { return Stand::default() };
        let most = choice.most.max(choice.least);
        if choice.done.is_some() {
            return Stand { passed: choice.least, planned: 0.0, least: choice.least, most };
        }
        let spare = self.spare.iter().filter(|taken| self.choices.iter().position(|other| other.list_of(&taken.id).is_some()) == Some(at));
        Stand::of(choice.taken.iter().chain(spare), choice.least, most)
    }

    /// The modules of the study on a choice's lists that count for none, there.
    pub fn spare_of(&self, at: usize) -> Vec<&Taken> {
        self.spare.iter().filter(|taken| self.choices.iter().position(|other| other.list_of(&taken.id).is_some()) == Some(at)).collect()
    }
}

/// A module of the study: passed, or planned and not failed.
struct Held {
    taken: Taken,
    /// The area of the progress it counts towards.
    area: Option<usize>,
    /// The row of the plan the student planned it for (`Planned::counts`), in the semester of
    /// `taken`.
    counts: Option<RowRef>,
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
        let exception = study.semester(semester).and_then(|s| s.items.iter().find(|item| item.module_id() == Some(id))).is_some_and(|item| item.exception)
            || study.semesters.iter().flat_map(|s| s.items.iter()).any(|item| item.module_id() == Some(id) && item.exception);
        Taken { id: id.to_string(), name: row.map_or_else(|| id.to_string(), |row| row.title.clone()), credits: row.and_then(|row| row.credits), semester, passed, exception }
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

    let mut choices = choices_of(input, study, &rows, &mut lines);
    let groups = groups_of(&rows, &choices);
    let (spare, pflicht) = assign(input, study, &rows, &lines, &mut choices, &groups);
    let mut view = PlanView { lines, choices, groups, spare, pflicht, to_choose: 0.0 };
    view.to_choose = to_choose(&rows, &view);
    view
}

/// The choices the regulation sums over: those tied by one sum (`study::choice_total`, two at
/// least, each with lists of its own), and the choices of the same area whose lists are all of
/// theirs and nothing else.
fn groups_of(rows: &[PlanRow], choices: &[Choice]) -> Vec<Group> {
    let mut groups: Vec<(&str, i64, Group)> = Vec::new();
    for (at, choice) in choices.iter().enumerate() {
        let Some((caption, total)) = choice.lines.iter().filter_map(|i| rows.get(*i)).find_map(|row| study::choice_total(row).map(|total| (row.variant.full.as_str(), total))) else { continue };
        if choice.lists.is_empty() || choice.fues {
            continue;
        }
        match groups.iter_mut().find(|(c, ord, _)| *c == caption && *ord == total.ord) {
            Some((.., group)) => group.members.push(at),
            None => {
                let label = total.label.trim();
                let label = label.strip_prefix("Summe ").or_else(|| label.strip_prefix("Sum ")).unwrap_or(label).trim().to_string();
                let together = choice.together.as_ref().map_or(total.credits, |(credits, _)| *credits);
                groups.push((caption, total.ord, Group { members: vec![at], extras: Vec::new(), together, label }));
            }
        }
    }
    let mut groups: Vec<Group> = groups.into_iter().map(|(.., group)| group).filter(|group| group.members.len() > 1).collect();
    for group in &mut groups {
        let ids = |at: &usize| choices.get(*at).map(|choice| choice.lists.iter().map(|list| list.id).collect::<BTreeSet<i64>>()).unwrap_or_default();
        let theirs: BTreeSet<i64> = group.members.iter().flat_map(ids).collect();
        let area = group.members.first().and_then(|at| choices.get(*at)).and_then(|choice| choice.area);
        group.extras = (0..choices.len())
            .filter(|at| !group.members.contains(at))
            .filter(|at| choices.get(*at).is_some_and(|choice| choice.area == area && !choice.fues && choice.lists.len() > 1 && ids(at) == theirs))
            .collect();
    }
    groups
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

/// The choices of the plan's rows (`lines`, which learn theirs), without what the study holds of
/// them (`assign`).
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

    choices
}

/// What the study holds of the choices, and what of it is on their lists but counts for none
/// (`PlanView::spare`); the modules counted for a row that is no choice (`PlanView::pflicht`).
///
/// The modules that fill a row's placeholders count there, then those planned for one of its rows
/// (`Planned::counts`: the workbench's „+ Einplanen", „Zählt für"), whether its lists have them or
/// not. Each of the others goes where it fits best (owner, 2026-11-08: „dass automatisch der
/// sinnvollste bereich gewählt wird. Wie zum beispiel wenn ein modul in zwei bereichen sein kann
/// aber einer davon schon genug credits hat, dann in den anderen bereich"), what is passed first,
/// then by semester: among the choices whose lists have it (the FÜS for a module the study counts
/// there), one that still asks for credits before one that only has room for them; of those, one
/// of the area the study counts it in; then the one with the shortest lists, the most specific
/// („Komplex Praktische Informatik" before the „Wahlpflicht: Komplex … / … / …" of the 4. FS that
/// takes from all three Komplexe, „Praktische Mathematik" before the Anwendungsfach); then the
/// first in the plan. The members of a group (`Group`) have the room of its sum together: past
/// it, what is left goes to its extras. Where none has room, a module counted there by the same
/// rule makes way for it if another choice has room for that one.
fn assign(input: &Input, study: &Study, rows: &[PlanRow], lines: &[PlanLine], choices: &mut [Choice], groups: &[Group]) -> (Vec<Taken>, Vec<(Taken, usize)>) {
    let doc = input.doc;
    let held = held_of(study, rows, doc);
    let mut counted: BTreeSet<String> = BTreeSet::new();
    for choice in choices.iter_mut() {
        for filler in choice.lines.iter().filter_map(|i| lines.get(*i)).flat_map(|line| line.fillers.iter()) {
            if counted.insert(filler.id.clone()) {
                choice.taken.push(filler.clone());
            }
        }
    }
    for choice in choices.iter_mut() {
        let own: Vec<(&str, i64)> = choice.lines.iter().filter_map(|i| lines.get(*i)).map(|line| (line.caption.as_str(), line.ord)).collect();
        for module in &held {
            let chosen = module.counts.as_ref().is_some_and(|row| own.iter().any(|(caption, ord)| row.is(input.program_id, caption, *ord)));
            if chosen && counted.insert(module.taken.id.clone()) {
                choice.taken.push(module.taken.clone());
            }
        }
    }
    // Counted for a row that is no choice: its area's, no choice's.
    let mut pflicht: Vec<(Taken, usize)> = Vec::new();
    for module in &held {
        let row = module.counts.as_ref().and_then(|row| lines.iter().position(|line| line.choice.is_none() && row.is(input.program_id, &line.caption, line.ord)));
        if let Some(at) = row {
            if counted.insert(module.taken.id.clone()) {
                pflicht.push((module.taken.clone(), at));
            }
        }
    }

    let fues_area = study.progress.iter().position(|p| p.area.kind == AreaKind::Fues);
    // What does not change while modules are counted: each choice's lists, whether it is the FÜS,
    // its area, where it begins.
    let lists: Vec<BTreeSet<String>> = choices.iter().map(|choice| choice.lists.iter().flat_map(|list| list.modules.iter()).cloned().collect()).collect();
    let fixed: Vec<(bool, Option<usize>, u8)> = choices.iter().map(|choice| (choice.fues, choice.area, choice.first)).collect();
    let member_of = |at: usize| groups.iter().position(|group| group.members.contains(&at));
    let most_of = |choice: &Choice| choice.most.max(choice.least);
    let taken_of = |choices: &[Choice], at: usize| choices.get(at).map_or(0.0, Choice::taken_credits);
    let members_taken = |choices: &[Choice], group: usize| groups.get(group).map_or(0.0, |group| group.members.iter().map(|at| taken_of(choices, *at)).sum::<f64>());
    // Whether choice `at` still asks for credits: its least, or as a member the group's sum.
    let needs = |choices: &[Choice], at: usize| -> bool {
        let Some(choice) = choices.get(at) else { return false };
        if choice.done.is_some() {
            return false;
        }
        let taken = choice.taken_credits();
        taken + EPSILON < choice.least
            || member_of(at).is_some_and(|group| taken + EPSILON < most_of(choice) && members_taken(choices, group) + EPSILON < groups.get(group).map_or(0.0, |group| group.together))
    };
    // Whether it has room left: below its most, a member below the group's sum as well.
    let room = |choices: &[Choice], at: usize| -> bool {
        let Some(choice) = choices.get(at) else { return false };
        needs(choices, at)
            || (choice.taken_credits() + EPSILON < most_of(choice) && member_of(at).is_none_or(|group| members_taken(choices, group) + EPSILON < groups.get(group).map_or(0.0, |group| group.together)))
    };
    let fits = |at: usize, module: &Held| -> bool {
        match fixed.get(at) {
            Some((true, ..)) => module.area.is_some() && module.area == fues_area,
            Some(_) => lists.get(at).is_some_and(|list| list.contains(&module.taken.id)),
            None => false,
        }
    };
    // The most fitting first, of the choices that fit `module`.
    let ranked = |choices: &[Choice], module: &Held| -> Vec<usize> {
        let mut fitting: Vec<usize> = (0..choices.len()).filter(|at| fits(*at, module)).collect();
        fitting.sort_by_key(|at| {
            let (_, area, first) = fixed.get(*at).copied().unwrap_or((false, None, u8::MAX));
            let other_area = module.area.is_some() && area != module.area;
            let size = lists.get(*at).map_or(usize::MAX, BTreeSet::len);
            (!needs(choices, *at), other_area, size, first, *at)
        });
        fitting
    };
    // Where each module went by the rule: those may make way.
    let mut by_rule: Vec<(usize, usize)> = Vec::new();
    for (index, module) in held.iter().enumerate() {
        if counted.contains(&module.taken.id) {
            continue;
        }
        let order = ranked(choices, module);
        if let Some(at) = order.iter().copied().find(|at| room(choices, *at)) {
            if let Some(choice) = choices.get_mut(at) {
                choice.taken.push(module.taken.clone());
            }
            counted.insert(module.taken.id.clone());
            by_rule.push((index, at));
            continue;
        }
        // None has room: one there by the rule moves on to another choice with room for it.
        'way: for at in order {
            for slot in 0..by_rule.len() {
                let Some(&(other, there)) = by_rule.get(slot) else { continue };
                let Some(moved) = held.get(other) else { continue };
                if there != at {
                    continue;
                }
                let Some(place) = choices.get(at).and_then(|choice| choice.taken.iter().position(|taken| taken.id == moved.taken.id)) else { continue };
                let Some(taken) = choices.get_mut(at).map(|choice| choice.taken.remove(place)) else { continue };
                let elsewhere = ranked(choices, moved).into_iter().find(|to| *to != at && room(choices, *to));
                match elsewhere {
                    Some(to) if room(choices, at) => {
                        if let Some(choice) = choices.get_mut(to) {
                            choice.taken.push(taken);
                        }
                        if let Some(choice) = choices.get_mut(at) {
                            choice.taken.push(module.taken.clone());
                        }
                        if let Some(entry) = by_rule.get_mut(slot) {
                            entry.1 = to;
                        }
                        counted.insert(module.taken.id.clone());
                        by_rule.push((index, at));
                        break 'way;
                    }
                    _ => {
                        if let Some(choice) = choices.get_mut(at) {
                            choice.taken.insert(place, taken);
                        }
                    }
                }
            }
        }
    }
    // A group's extra still asking for credits takes them from its members where these hold more
    // than the group's sum asks and each keeps its least: Grundlagen 24, Praktische 18, Angewandte
    // 12 is the 50 of Informatik's Komplexe, its Wahlpflicht of the 4. FS among them. The last
    // counted by the rule goes first.
    for (at_group, group) in groups.iter().enumerate() {
        for &extra in &group.extras {
            loop {
                let lacking = choices.get(extra).is_some_and(|choice| choice.done.is_none() && choice.taken_credits() + EPSILON < choice.least);
                if !lacking {
                    break;
                }
                let surplus = members_taken(choices, at_group) - group.together;
                let found = by_rule.iter().enumerate().rev().find_map(|(slot, (index, there))| {
                    let module = held.get(*index)?;
                    let credits = module.taken.credits.unwrap_or(0.0);
                    let member = choices.get(*there)?;
                    let keeps = member.taken_credits() - credits + EPSILON >= member.least;
                    (group.members.contains(there) && credits <= surplus + EPSILON && keeps && lists.get(extra).is_some_and(|list| list.contains(&module.taken.id))).then_some((slot, *there))
                });
                let Some((slot, there)) = found else { break };
                let id = by_rule.get(slot).and_then(|(index, _)| held.get(*index)).map(|module| module.taken.id.clone()).unwrap_or_default();
                let taken = choices.get_mut(there).and_then(|choice| choice.taken.iter().position(|taken| taken.id == id).map(|at| choice.taken.remove(at)));
                match (taken, choices.get_mut(extra)) {
                    (Some(taken), Some(choice)) => choice.taken.push(taken),
                    _ => break,
                }
                if let Some(entry) = by_rule.get_mut(slot) {
                    entry.1 = extra;
                }
            }
        }
    }
    // What a choice takes, in the order of the study: passed first, then by semester.
    for choice in choices.iter_mut() {
        choice.taken.sort_by_key(|taken| (!taken.passed, taken.semester));
    }
    let listed: BTreeSet<&str> = choices.iter().filter(|choice| !choice.fues).flat_map(|choice| choice.modules()).collect();
    let spare = held.iter().filter(|module| !counted.contains(&module.taken.id) && listed.contains(module.taken.id.as_str())).map(|module| module.taken.clone()).collect();
    (spare, pflicht)
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
    lists_for(input.tree, input.placements, row, area.and_then(|area| study.progress.get(area)).map(|p| &p.area.tree))
}

/// The lists of a choice of `row` (`lists_of`), its area's tree being `claimed` (`study::Area::tree`).
pub(crate) fn lists_for(tree: &[CatalogArea], placements: &[(String, i64)], row: &PlanRow, claimed: Option<&BTreeSet<i64>>) -> Vec<List> {
    let found = plan::areas_for_row(row.entry, &row.variant.full, tree, &row.variant.entries);
    let mut tops: Vec<&CatalogArea> = found.areas.iter().filter_map(|found| tree.iter().find(|area| area.id == found.id)).collect();
    if tops.is_empty() {
        // The tree's areas of the progress's area where a student chooses, the topmost of them.
        let claimed: Vec<&CatalogArea> = claimed.map(|claimed| tree.iter().filter(|area| claimed.contains(&area.id) && area.choice).collect()).unwrap_or_default();
        tops = claimed.iter().copied().filter(|area| !claimed.iter().any(|top| study::below(area, top))).collect();
    }
    tops.into_iter()
        .map(|top| {
            let mut seen = BTreeSet::new();
            let modules = placements
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
fn held_of(study: &Study, rows: &[PlanRow], doc: &PlanDoc) -> Vec<Held> {
    let named: BTreeSet<&str> = rows.iter().filter_map(|row| row.entry.module_id.as_deref()).collect();
    let mut held: Vec<Held> = Vec::new();
    for semester in &study.semesters {
        for item in semester.items.iter().filter(|item| item.passed || !item.failed) {
            let Some(id) = item.module_id() else { continue };
            if named.contains(id) {
                continue;
            }
            let taken = Taken { id: id.to_string(), name: item.name.clone(), credits: item.credits, semester: semester.key, passed: item.passed, exception: item.exception };
            let counts = doc.counts_of(semester.key, id).cloned();
            match held.iter_mut().find(|known| known.taken.id == id) {
                Some(known) if item.passed && !known.taken.passed => {
                    known.taken = taken;
                    known.counts = counts;
                }
                Some(_) => {}
                None => held.push(Held { area: item.area, taken, counts }),
            }
        }
    }
    held.sort_by_key(|module| (!module.taken.passed, module.taken.semester));
    held
}

/// What the choices still ask for together: a group as `Group::open`, a sum the regulation prints
/// over several otherwise counted once, less what is taken of all of them.
fn to_choose(rows: &[PlanRow], view: &PlanView) -> f64 {
    let choices = &view.choices;
    let mut sum: f64 = view.groups.iter().map(|group| group.open(view)).sum();
    let mut totals: Vec<(&str, i64)> = Vec::new();
    for (at, choice) in choices.iter().enumerate() {
        if view.group_of(at).is_some() {
            continue;
        }
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

    /// A program of the snapshot, as „Mein Studium" works its study out.
    struct Program {
        id: String,
        variants: Vec<crate::variants::PlanVariant>,
        tree: Vec<CatalogArea>,
        placements: Vec<(String, i64)>,
        fues: Vec<String>,
        rows: Vec<folia_model::rows::CatalogRow>,
        leave: BTreeSet<SemesterKey>,
    }

    fn program(id: &str) -> Program {
        use folia_model::labels::{Labelled, OfferStatus};
        let db = folia_test_support::open();
        let entries = folia_query::program_plan_entries(&db, id).unwrap();
        let variants = crate::variants::plan_variants(&entries, &folia_query::program_plan_totals(&db, id).unwrap(), folia_locale::Locale::De);
        let placed = folia_query::program_areas(&db, id).unwrap();
        let tree = crate::areas::catalog_areas(&placed, &folia_query::program_area_tree(&db, id).unwrap());
        let placements: Vec<(String, i64)> = placed.iter().map(|p| (p.module_id.clone(), p.area_id)).collect();
        let fues = folia_query::program_modules(&db, id, folia_routes::filter::ProgramRelation::Fues).unwrap().into_iter().map(|m| m.module_id).collect();
        let mut ids: Vec<String> = entries.iter().filter_map(|entry| entry.module_id.clone()).chain(placements.iter().map(|(module, _)| module.clone())).filter(|id| url::is_module_id(id)).collect();
        ids.sort();
        ids.dedup();
        let query = folia_routes::filter::CatalogQuery { only_ids: Some(ids.clone()), offer: Some(OfferStatus::ALL.to_vec()), ..Default::default() };
        let rows = folia_query::catalog_page(&db, &query, 0, ids.len() as u64 + 1).unwrap().rows;
        Program { id: id.to_string(), variants, tree, placements, fues, rows, leave: BTreeSet::new() }
    }

    impl Program {
        fn view(&self, doc: &PlanDoc) -> (Study, PlanView) {
            let input = Input {
                program_id: &self.id,
                core: self.variants.first(),
                page: None,
                start: key("2025W"),
                now: key("2026W"),
                leave: &self.leave,
                until: None,
                doc,
                rows: &self.rows,
                tree: &self.tree,
                placements: &self.placements,
                fues: &self.fues,
            };
            let study = study::study(&input);
            let view = plan_view(&input, &study);
            (study, view)
        }
    }

    /// The modules each choice takes, by name.
    fn taken(view: &PlanView, name: &str) -> Vec<String> {
        view.choices.iter().find(|choice| choice.name.starts_with(name)).map(|choice| choice.taken.iter().map(|taken| taken.id.clone()).collect()).unwrap_or_default()
    }

    fn planned(lines: &[(&str, &str)]) -> PlanDoc {
        let mut doc = PlanDoc::default();
        for (s, id) in lines {
            doc.plan(key(s), id, 1, None);
        }
        doc
    }

    /// Informatik B.Sc. (owner, 2026-11-08: „module nicht richtig angerechnet werden, weil zum
    /// beispiel in wahlpflicht landen, anstatt in dem konkreten komplex"): Datenbanken lies in the
    /// Komplex Praktische Informatik and counts there, not for the „Wahlpflicht: Komplex … / … /
    /// …" of the 4. FS that takes from all three, which the plan names first.
    #[test]
    fn a_module_counts_for_its_komplex_not_for_the_wahlpflicht_over_all_three() {
        let informatik = program("079-82-2008");
        let (_, view) = informatik.view(&planned(&[("2027S", "12330")]));
        assert_eq!(taken(&view, "Komplex Praktische Informatik"), ["12330"]);
        assert!(taken(&view, "Wahlpflicht: Komplex").is_empty());
        // Approximationsalgorithmen of the Grundlagen der Informatik likewise.
        let (_, view) = informatik.view(&planned(&[("2027S", "12330"), ("2027W", "12329")]));
        assert_eq!(taken(&view, "Komplex Grundlagen der Informatik"), ["12329"]);
        assert!(taken(&view, "Wahlpflicht: Komplex").is_empty());
    }

    /// Past the sum of the Komplexe (44) what is left goes to the Wahlpflicht of the 4. FS; and
    /// where a Komplex holds more than its least and the three more than their sum, the
    /// Wahlpflicht takes from it: 24, 18 and 12 are the 50 the four ask for together.
    #[test]
    fn the_wahlpflicht_over_all_three_takes_what_the_komplexe_hold_beyond_their_sum() {
        let informatik = program("079-82-2008");
        let gi = [("2027S", "12329"), ("2027S", "12350"), ("2027S", "11450")];
        let pi = [("2027W", "14012"), ("2027W", "13569"), ("2027W", "14460")];
        let ati = [("2028S", "12339"), ("2028S", "12882")];
        let all: Vec<(&str, &str)> = gi.iter().chain(&pi).chain(&ati).copied().collect();
        let (_, view) = informatik.view(&planned(&all));
        assert_eq!(taken(&view, "Komplex Grundlagen der Informatik"), ["12329", "12350", "11450"]);
        assert_eq!(taken(&view, "Komplex Praktische Informatik"), ["14012", "13569"]);
        assert_eq!(taken(&view, "Komplex Angewandte und Technische Informatik"), ["12339", "12882"]);
        assert_eq!(taken(&view, "Wahlpflicht: Komplex"), ["14460"], "the last a Komplex could give away");
        for name in ["Komplex Grundlagen der Informatik", "Komplex Praktische Informatik", "Komplex Angewandte und Technische Informatik", "Wahlpflicht: Komplex"] {
            assert!(view.choices.iter().find(|choice| choice.name.starts_with(name)).is_some_and(Choice::complete), "{name}");
        }
        assert_eq!(view.groups[0].open(&view), 0.0);
    }

    /// A module two choices list goes where it is still asked for (owner, 2026-11-08: „wenn ein
    /// modul in zwei bereichen sein kann aber einer davon schon genug credits hat, dann in den
    /// anderen bereich"): Algorithmische Graphentheorie and Kombinatorik are listed by „Praktische
    /// Mathematik" (6 LP) and the Anwendungsfach (its Mathematik): the first fills the first, the
    /// second goes to the Anwendungsfach. Statistik für Anwender is listed by Praktische Mathematik
    /// alone: planned after Graphentheorie, it takes the place there, and Graphentheorie makes way.
    #[test]
    fn a_module_two_choices_list_goes_where_it_is_still_asked_for() {
        let informatik = program("079-82-2008");
        let (_, view) = informatik.view(&planned(&[("2027S", "11405"), ("2027W", "11409")]));
        assert_eq!(taken(&view, "Modul aus dem Bereich Praktische Mathematik"), ["11405"]);
        assert_eq!(taken(&view, "Anwendungsfach"), ["11409"]);
        let (_, view) = informatik.view(&planned(&[("2027S", "11405"), ("2027W", "11926")]));
        assert_eq!(taken(&view, "Modul aus dem Bereich Praktische Mathematik"), ["11926"]);
        assert_eq!(taken(&view, "Anwendungsfach"), ["11405"]);
        assert!(view.spare.is_empty());
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
