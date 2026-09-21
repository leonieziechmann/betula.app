//! URLs of the app, in one place for the server, the pages and the tests.
//!
//! `/`                                  landing page
//! `/catalog?…`                         module catalog; the query string is a `CatalogQuery`
//! `/catalog/module/<id>`               module page
//! `/bookmarks?…`                       the visitor's marked modules (`BookmarksUrl`); which ones
//!                                      they are is never part of a URL, only how they are shown
//! `/programs`                          program overview
//! `/programs/<slug>[/plan|areas|modules][?variant=<n>][&open=<id>][&full=1]`   program page, its
//!                                      tabs, which of several study plans is shown, which module
//!                                      stands beside it, and whether that module fills the page
//!                                      (`ProgramUrl`)
//!
//! Most filters can also exclude: `exam=written&not-exam=presentation` lists modules with a
//! written exam and without a presentation. A value that is both included and excluded counts
//! as included.
//!
//! The catalog parameters are readable and tolerant: a plain HTML form (no JavaScript) sends
//! repeated parameters and empty values, the app writes the canonical form, a hand-edited
//! link may contain nonsense. Unknown names and values are ignored, never an error.

use crate::filter::{
    CatalogQuery, ExamPart, KindFilter, Language, PlanSemesterFilter, ProgramRelation, ProgramScope, SortKey,
    TurnusFilter,
};
use crate::labels::{Campus, ExamForm, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity};

pub const HOME: &str = "/";
pub const CATALOG: &str = "/catalog";
pub const PROGRAMS: &str = "/programs";
/// „Merkliste": the modules the visitor has marked. The list itself lives in the browser.
pub const BOOKMARKS: &str = "/bookmarks";

pub fn module_path(id: &str) -> String {
    format!("/catalog/module/{}", encode(id))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProgramTab {
    /// Regelstudienplan
    #[default]
    Plan,
    /// Wahlpflicht & Bereiche
    Areas,
    /// Alle Module
    Modules,
}

impl ProgramTab {
    pub const ALL: &'static [Self] = &[Self::Plan, Self::Areas, Self::Modules];

    pub fn segment(self) -> &'static str {
        match self {
            ProgramTab::Plan => "plan",
            ProgramTab::Areas => "areas",
            ProgramTab::Modules => "modules",
        }
    }

    pub fn from_segment(segment: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|tab| tab.segment() == segment)
    }

    pub fn label(self) -> &'static str {
        match self {
            ProgramTab::Plan => "Regelstudienplan",
            ProgramTab::Areas => "Wahlpflicht & Bereiche",
            ProgramTab::Modules => "Alle Module",
        }
    }
}

pub fn program_path(slug: &str, tab: ProgramTab) -> String {
    format!("/programs/{}/{}", encode(slug), tab.segment())
}

/// How many study plans of one program can be told apart in the URL. A program has one plan per
/// study direction; the highest seen so far is eight.
pub const MAX_PLAN_VARIANTS: usize = 20;
/// How many rows of one study plan can be told apart in the URL (the longest has 29).
pub const MAX_PLAN_ROWS: usize = 200;

/// What a program's address says besides the program itself: which view, which of its study
/// plans, and which of its modules is shown next to it.
///
/// Both are content, not personal view settings: a link leads to the plan and the module it
/// shows, and both work without JavaScript. How the plan is *drawn* is personal and stays out of
/// the URL (R13).
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramUrl {
    pub slug: String,
    pub tab: ProgramTab,
    /// 1-based. The first plan is the default and is not written.
    pub variant: usize,
    /// The module shown beside the page (`open=<id>`); its own page is `module_path`.
    pub open: Option<String>,
    /// The area shown beside the page (`area=<id>`): what the program's tree puts in it. A
    /// module opened from that list keeps it, so closing the module returns to the area.
    pub area: Option<i64>,
    /// The row of the study plan shown beside the page (`req=<n>`, 1-based within the chosen
    /// plan). Most of these rows are requirements the plan states without naming a module
    /// („Wahlpflichtmodule der Studienrichtung"), so they have nothing else to be named by.
    pub req: Option<usize>,
    /// The module of `open` fills the page (`full=1`): the module's own page, shown inside the
    /// program's area, so that „Vollbild" neither changes the area nor the tab. Nothing without
    /// `open`.
    pub full: bool,
}

impl ProgramUrl {
    pub fn new(slug: &str, tab: ProgramTab) -> Self {
        Self { slug: slug.to_string(), tab, variant: 1, open: None, area: None, req: None, full: false }
    }

    pub fn parse(slug: &str, tab: ProgramTab, raw_query: &str) -> Self {
        let pairs = parse_pairs(raw_query);
        let first = |name: &str| pairs.iter().find(|(key, _)| key == name).map(|(_, value)| value.trim().to_string());
        let open = first("open").filter(|id| is_module_id(id));
        Self {
            slug: slug.to_string(),
            tab,
            variant: first("variant").and_then(|value| value.parse::<usize>().ok()).filter(|n| (1..=MAX_PLAN_VARIANTS).contains(n)).unwrap_or(1),
            full: open.is_some() && first("full").as_deref() == Some("1"),
            open,
            area: first("area").and_then(|value| value.parse::<i64>().ok()).filter(|id| *id > 0),
            req: first("req").and_then(|value| value.parse::<usize>().ok()).filter(|n| (1..=MAX_PLAN_ROWS).contains(n)),
        }
    }

    /// One spelling per page, which is also the key the server caches it under.
    pub fn query(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        if self.variant > 1 {
            out.push(format!("variant={}", self.variant));
        }
        if let Some(id) = self.area {
            out.push(format!("area={id}"));
        }
        if let Some(row) = self.req {
            out.push(format!("req={row}"));
        }
        if let Some(id) = &self.open {
            out.push(format!("open={}", encode(id)));
            if self.full {
                out.push("full=1".to_string());
            }
        }
        match out.is_empty() {
            true => String::new(),
            false => format!("?{}", out.join("&")),
        }
    }

    pub fn path(&self) -> String {
        format!("{}{}", program_path(&self.slug, self.tab), self.query())
    }

    /// The same page with this module beside it, or (`None`) without it. Beside it, not filling
    /// it: what fills the page is asked for with `with_full`.
    pub fn with_open(&self, id: Option<&str>) -> Self {
        Self { open: id.map(str::to_string), full: false, ..self.clone() }
    }

    /// The module beside the page fills it (`true`), or stands beside it again (`false`).
    pub fn with_full(&self, full: bool) -> Self {
        Self { full: full && self.open.is_some(), ..self.clone() }
    }

    /// The same page with this area beside it; what was shown so far makes way for it.
    pub fn with_area(&self, id: Option<i64>) -> Self {
        Self { area: id, open: None, req: None, full: false, ..self.clone() }
    }

    /// The same page with this row of the study plan beside it.
    pub fn with_req(&self, row: Option<usize>) -> Self {
        Self { req: row, open: None, area: None, full: false, ..self.clone() }
    }

    pub fn with_variant(&self, variant: usize) -> Self {
        Self { variant, ..self.clone() }
    }

    pub fn with_tab(&self, tab: ProgramTab) -> Self {
        Self { tab, ..self.clone() }
    }
}

/// A group of degree levels, as the program overview filters them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LevelGroup {
    Bachelor,
    Master,
    /// Lehramt, Bachelor and Master.
    Teaching,
    Doctoral,
    /// Without a degree, or another one.
    Other,
}

impl LevelGroup {
    pub const ALL: &'static [Self] = &[Self::Bachelor, Self::Master, Self::Teaching, Self::Doctoral, Self::Other];

    pub fn code(self) -> &'static str {
        match self {
            LevelGroup::Bachelor => "bachelor",
            LevelGroup::Master => "master",
            LevelGroup::Teaching => "teaching",
            LevelGroup::Doctoral => "doctoral",
            LevelGroup::Other => "other",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LevelGroup::Bachelor => "Bachelor",
            LevelGroup::Master => "Master",
            LevelGroup::Teaching => "Lehramt",
            LevelGroup::Doctoral => "Promotion",
            LevelGroup::Other => "Sonstige",
        }
    }

    pub fn of(level: &crate::labels::Code<crate::labels::DegreeLevel>) -> Self {
        use crate::labels::DegreeLevel;
        match level.known() {
            Some(DegreeLevel::Bachelor) => LevelGroup::Bachelor,
            Some(DegreeLevel::Master) => LevelGroup::Master,
            Some(DegreeLevel::TeachingBachelor | DegreeLevel::TeachingMaster) => LevelGroup::Teaching,
            Some(DegreeLevel::Doctoral) => LevelGroup::Doctoral,
            Some(DegreeLevel::None | DegreeLevel::Other) | None => LevelGroup::Other,
        }
    }
}

/// A group of forms of study, as the program overview filters them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormGroup {
    Dual,
    DoubleDegree,
    /// Part time and distance learning.
    Flexible,
}

impl FormGroup {
    pub const ALL: &'static [Self] = &[Self::Dual, Self::DoubleDegree, Self::Flexible];

    pub fn code(self) -> &'static str {
        match self {
            FormGroup::Dual => "dual",
            FormGroup::DoubleDegree => "double",
            FormGroup::Flexible => "flexible",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FormGroup::Dual => "Dual",
            FormGroup::DoubleDegree => "Doppelabschluss",
            FormGroup::Flexible => "Teilzeit & Fern",
        }
    }

    pub fn of(variant: Option<&crate::labels::Code<crate::labels::StudyVariant>>) -> Option<Self> {
        use crate::labels::StudyVariant;
        match variant?.known()? {
            StudyVariant::DualPractice | StudyVariant::DualTraining => Some(FormGroup::Dual),
            StudyVariant::DoubleDegree => Some(FormGroup::DoubleDegree),
            StudyVariant::PartTime | StudyVariant::Distance => Some(FormGroup::Flexible),
            StudyVariant::Extended | StudyVariant::Reduced | StudyVariant::Other => None,
        }
    }
}

/// What the URL of the program overview says: `/programs?q=…&level=bachelor,master&form=dual&plan=1`.
/// Tolerant and canonical like `CatalogUrl`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgramsUrl {
    /// The search of the top bar.
    pub text: String,
    /// Any of these; empty means all.
    pub levels: Vec<LevelGroup>,
    /// Any of these; empty means all.
    pub forms: Vec<FormGroup>,
    /// Only programs with a validated study plan.
    pub with_plan: bool,
}

impl ProgramsUrl {
    pub fn parse(raw_query: &str) -> Self {
        let pairs = parse_pairs(raw_query);
        let codes = |name: &str| -> Vec<String> {
            pairs.iter().filter(|(key, _)| key == name).flat_map(|(_, value)| value.split(',')).map(|code| code.trim().to_ascii_lowercase()).collect()
        };
        let (levels, forms) = (codes("level"), codes("form"));
        Self {
            text: pairs.iter().find(|(key, value)| key == "q" && !value.trim().is_empty()).map(|(_, value)| value.trim().to_string()).unwrap_or_default(),
            levels: LevelGroup::ALL.iter().copied().filter(|level| levels.iter().any(|code| code == level.code())).collect(),
            forms: FormGroup::ALL.iter().copied().filter(|form| forms.iter().any(|code| code == form.code())).collect(),
            with_plan: pairs.iter().any(|(key, value)| key == "plan" && value == "1"),
        }
    }

    pub fn is_filtered(&self) -> bool {
        !self.text.is_empty() || !self.levels.is_empty() || !self.forms.is_empty() || self.with_plan
    }

    pub fn path(&self) -> String {
        let mut out: Vec<(&str, String)> = Vec::new();
        if !self.text.trim().is_empty() {
            out.push(("q", self.text.trim().to_string()));
        }
        if !self.levels.is_empty() {
            out.push(("level", self.levels.iter().map(|level| level.code()).collect::<Vec<_>>().join(",")));
        }
        if !self.forms.is_empty() {
            out.push(("form", self.forms.iter().map(|form| form.code()).collect::<Vec<_>>().join(",")));
        }
        if self.with_plan {
            out.push(("plan", "1".to_string()));
        }
        if out.is_empty() {
            return PROGRAMS.to_string();
        }
        format!("{PROGRAMS}?{}", out.iter().map(|(key, value)| format!("{key}={}", encode(value))).collect::<Vec<_>>().join("&"))
    }
}

/// What a module id may look like wherever one arrives from outside (a URL, the browser's
/// storage): it ends up in links and queries.
pub fn is_module_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// How the list of marked modules is ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BookmarkSort {
    /// The newest mark first: the order of a list one keeps adding to.
    #[default]
    Added,
    Title,
    Credits,
    /// Number of teaching events in the module's newest semester.
    Events,
}

impl BookmarkSort {
    pub const ALL: &'static [Self] = &[Self::Added, Self::Title, Self::Credits, Self::Events];

    pub fn code(self) -> &'static str {
        match self {
            BookmarkSort::Added => "added",
            BookmarkSort::Title => "title",
            BookmarkSort::Credits => "ects",
            BookmarkSort::Events => "events",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BookmarkSort::Added => "Zuletzt gemerkt",
            BookmarkSort::Title => "Titel",
            BookmarkSort::Credits => "Leistungspunkte",
            BookmarkSort::Events => "Termine",
        }
    }
}

/// The half of the year a module is offered in, as the list of marked modules filters it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Season {
    Winter,
    Summer,
}

impl Season {
    pub const ALL: &'static [Self] = &[Self::Winter, Self::Summer];

    pub fn code(self) -> &'static str {
        match self {
            Season::Winter => "winter",
            Season::Summer => "summer",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Season::Winter => "Winter",
            Season::Summer => "Sommer",
        }
    }
}

/// What the URL of the marked modules says: `/bookmarks?turnus=winter&sort=ects&desc=1&open=11101`.
/// It says how the list is shown and never what is on it: the marks are personal, they live in
/// the browser and reach neither a URL nor the server (R9, R13). Tolerant and canonical like
/// `CatalogUrl`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookmarksUrl {
    /// Only what is offered in this half of the year.
    pub season: Option<Season>,
    pub sort: BookmarkSort,
    /// The other way round. The order of marking has one direction only, the newest first.
    pub descending: bool,
    /// The module previewed next to the list (`open=<id>`), as in the catalog.
    pub open: Option<String>,
}

impl BookmarksUrl {
    pub fn parse(raw_query: &str) -> Self {
        let pairs = parse_pairs(raw_query);
        let first = |name: &str| pairs.iter().find(|(key, value)| key == name && !value.trim().is_empty()).map(|(_, value)| value.trim().to_ascii_lowercase());
        let sort = first("sort").and_then(|code| BookmarkSort::ALL.iter().copied().find(|sort| sort.code() == code)).unwrap_or_default();
        Self {
            season: first("turnus").and_then(|code| Season::ALL.iter().copied().find(|season| season.code() == code)),
            sort,
            descending: sort != BookmarkSort::Added && first("desc").is_some(),
            open: pairs.iter().find(|(key, _)| key == "open").map(|(_, id)| id.trim().to_string()).filter(|id| is_module_id(id)),
        }
    }

    pub fn path(&self) -> String {
        let mut out: Vec<(&str, String)> = Vec::new();
        if let Some(season) = self.season {
            out.push(("turnus", season.code().to_string()));
        }
        if self.sort != BookmarkSort::Added {
            out.push(("sort", self.sort.code().to_string()));
            if self.descending {
                out.push(("desc", "1".to_string()));
            }
        }
        if let Some(id) = &self.open {
            out.push(("open", id.clone()));
        }
        if out.is_empty() {
            return BOOKMARKS.to_string();
        }
        format!("{BOOKMARKS}?{}", out.iter().map(|(key, value)| format!("{key}={}", encode(value))).collect::<Vec<_>>().join("&"))
    }

    /// The same list with this module's preview open, or (`None`) with the preview closed.
    pub fn with_open(&self, id: Option<&str>) -> Self {
        Self { open: id.map(str::to_string), ..self.clone() }
    }
}

/// How many modules one catalog page lists.
pub const PAGE_SIZE: u64 = 50;

/// What a catalog URL says: the filter and the page (1-based).
#[derive(Clone, Debug, PartialEq)]
pub struct CatalogUrl {
    pub query: CatalogQuery,
    pub page: u64,
    /// The module previewed next to the list (`open=<id>`). Its own page is `module_path`.
    pub open: Option<String>,
}

impl Default for CatalogUrl {
    fn default() -> Self {
        Self { query: CatalogQuery::default(), page: 1, open: None }
    }
}

impl CatalogUrl {
    pub fn offset(&self) -> u64 {
        (self.page.max(1) - 1) * PAGE_SIZE
    }

    /// Parses a raw query string (`a=b&c=d`, with or without the leading `?`).
    pub fn parse(raw_query: &str) -> Self {
        Self::from_pairs(&parse_pairs(raw_query))
    }

    pub fn from_pairs(pairs: &[(String, String)]) -> Self {
        // Codes may come repeated (`form=lecture&form=exercise`) or joined (`form=lecture,exercise`).
        let codes = |name: &str| -> Vec<String> {
            pairs
                .iter()
                .filter(|(key, _)| key == name)
                .flat_map(|(_, value)| value.split(','))
                .map(|code| code.trim().to_ascii_lowercase())
                .filter(|code| !code.is_empty())
                .collect()
        };
        let first = |name: &str| -> Option<String> {
            pairs.iter().find(|(key, value)| key == name && !value.trim().is_empty()).map(|(_, v)| v.trim().to_string())
        };
        let names = |name: &str| -> Vec<String> {
            let mut all: Vec<String> = pairs
                .iter()
                .filter(|(key, value)| key == name && !value.trim().is_empty())
                .map(|(_, value)| value.trim().to_string())
                .collect();
            all.dedup();
            all
        };
        let yes_no = |name: &str| match first(name).as_deref() {
            Some("yes") => Some(true),
            Some("no") => Some(false),
            _ => None,
        };
        fn known<E: Labelled + PartialEq>(codes: &[String]) -> Vec<E> {
            let mut values: Vec<E> = Vec::new();
            for value in codes.iter().filter_map(|code| E::from_code(code)) {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            values
        }

        fn without<T: PartialEq>(excluded: Vec<T>, included: &[T]) -> Vec<T> {
            excluded.into_iter().filter(|value| !included.contains(value)).collect()
        }
        let kinds_of = |name: &str| -> Vec<KindFilter> {
            let mut kinds: Vec<KindFilter> = known::<ModuleKind>(&codes(name)).into_iter().map(KindFilter::Stated).collect();
            if codes(name).iter().any(|code| code == "none") {
                kinds.push(KindFilter::Unstated);
            }
            kinds
        };
        let languages_of = |name: &str| -> Vec<Language> {
            let mut all: Vec<Language> = Vec::new();
            for language in codes(name).iter().filter_map(|code| Language::from_code(code)) {
                if !all.contains(&language) {
                    all.push(language);
                }
            }
            all
        };
        let exam_parts_of = |name: &str| -> Vec<ExamPart> {
            let chosen = codes(name);
            ExamPart::ALL.iter().copied().filter(|part| chosen.iter().any(|c| c == part.code())).collect()
        };

        let program = first("program").map(|program_slug| {
            let kinds = kinds_of("kind");
            ProgramScope {
                program_slug,
                relation: if first("list").as_deref() == Some("fues") {
                    ProgramRelation::Fues
                } else {
                    ProgramRelation::Curricular
                },
                plan_semester: match first("semester").as_deref() {
                    Some("none") => Some(PlanSemesterFilter::Unstated),
                    Some(n) => n.parse::<u8>().ok().filter(|n| (1..=20).contains(n)).map(PlanSemesterFilter::Semester),
                    None => None,
                },
                kinds_exclude: without(kinds_of("not-kind"), &kinds),
                kinds,
                area: first("area").and_then(|value| value.parse::<i64>().ok()).filter(|id| *id > 0),
                // Derived by the page loader, never read from an address.
                semester_areas: Vec::new(),
                semester_electives: false,
            }
        });

        let turnus_codes = codes("turnus");
        let not_turnus_codes = codes("not-turnus");
        let turnus = |code: &str| turnus_codes.iter().any(|c| c == code);
        let not_turnus = |code: &str| !turnus(code) && not_turnus_codes.iter().any(|c| c == code);
        let exam_codes = codes("exam");
        let status_codes = codes("status");
        let teaching_forms = known::<TeachingForm>(&codes("form"));
        let exam_parts = exam_parts_of("exam");
        let campuses = known::<Campus>(&codes("campus"));
        let languages = languages_of("lang");

        let query = CatalogQuery {
            text: first("q").unwrap_or_default(),
            program,
            lecturers_include: names("lecturer"),
            lecturers_exclude: names("not-lecturer"),
            department_id: first("department").and_then(|id| id.parse().ok()),
            turnus: TurnusFilter {
                winter: turnus("winter"),
                summer: turnus("summer"),
                irregular: turnus("irregular"),
                not_winter: not_turnus("winter"),
                not_summer: not_turnus("summer"),
                not_irregular: not_turnus("irregular"),
                year_parity: first("years").and_then(|code| TurnusParity::from_code(&code)),
            },
            teaching_forms_exclude: without(known::<TeachingForm>(&codes("not-form")), &teaching_forms),
            teaching_forms,
            duration_semesters: first("duration").and_then(|n| n.parse().ok()).filter(|n| (1..=12).contains(n)),
            limited: yes_no("limited"),
            fues: match first("fues").as_deref() {
                Some("only") => Some(true),
                Some("none") => Some(false),
                _ => None,
            },
            exam_forms: known::<ExamForm>(&exam_codes),
            exam_parts_exclude: without(exam_parts_of("not-exam"), &exam_parts),
            exam_parts,
            graded: yes_no("graded"),
            offer: if status_codes.iter().any(|code| code == "all") {
                Some(OfferStatus::ALL.to_vec())
            } else {
                Some(known::<OfferStatus>(&status_codes)).filter(|chosen| !chosen.is_empty())
            },
            credits_min: first("ects_min").and_then(|n| n.replace(',', ".").parse().ok()).filter(|n: &f64| n.is_finite()),
            credits_max: first("ects_max").and_then(|n| n.replace(',', ".").parse().ok()).filter(|n: &f64| n.is_finite()),
            campuses_exclude: without(known::<Campus>(&codes("not-campus")), &campuses),
            campuses,
            languages_exclude: without(languages_of("not-lang"), &languages),
            languages,
            only_ids: None,
            without_ids: Vec::new(),
            // The URL carries the switch; what is marked lives in the browser (R20).
            marked: match first("marked").as_deref() {
                Some("only") => Some(true),
                Some("none") => Some(false),
                _ => None,
            },
            // The same for the modules that are passed.
            prerequisites_met_by: (first("prereqs").as_deref() == Some("met")).then(Vec::new),
            sort: match first("sort").as_deref() {
                Some("id") => SortKey::Id,
                Some("ects") => SortKey::Credits,
                Some("events") => SortKey::Events,
                Some("title") => SortKey::Title,
                _ => SortKey::Default,
            },
            descending: first("desc").is_some(),
        };
        let page = first("page").and_then(|n| n.parse::<u64>().ok()).filter(|n| (1..=100_000).contains(n)).unwrap_or(1);
        let open = first("open").filter(|id| id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        Self { query, page, open }
    }

    /// The canonical query string, without `?`; empty for the default catalog.
    /// Equal filter states give equal strings, so it also serves as a cache key.
    pub fn to_query_string(&self) -> String {
        let q = &self.query;
        let mut out: Vec<(&str, String)> = Vec::new();
        let join = |codes: Vec<&str>| codes.join(",");

        if !q.text.trim().is_empty() {
            out.push(("q", q.text.trim().to_string()));
        }
        if let Some(scope) = &q.program {
            out.push(("program", scope.program_slug.clone()));
            if scope.relation == ProgramRelation::Fues {
                out.push(("list", "fues".to_string()));
            }
            match scope.plan_semester {
                Some(PlanSemesterFilter::Semester(n)) => out.push(("semester", n.to_string())),
                Some(PlanSemesterFilter::Unstated) => out.push(("semester", "none".to_string())),
                None => {}
            }
            if let Some(area) = scope.area {
                out.push(("area", area.to_string()));
            }
            for (name, kinds) in [("kind", &scope.kinds), ("not-kind", &scope.kinds_exclude)] {
                if !kinds.is_empty() {
                    let codes = kinds.iter().map(|kind| match kind {
                        KindFilter::Stated(kind) => kind.code(),
                        KindFilter::Unstated => "none",
                    });
                    out.push((name, join(codes.collect())));
                }
            }
        }
        out.extend(q.lecturers_include.iter().map(|name| ("lecturer", name.clone())));
        out.extend(q.lecturers_exclude.iter().map(|name| ("not-lecturer", name.clone())));
        if let Some(id) = q.department_id {
            out.push(("department", id.to_string()));
        }
        let turnus: Vec<&str> = [(q.turnus.winter, "winter"), (q.turnus.summer, "summer"), (q.turnus.irregular, "irregular")]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, code)| *code)
            .collect();
        if !turnus.is_empty() {
            out.push(("turnus", join(turnus)));
        }
        let not_turnus: Vec<&str> =
            [(q.turnus.not_winter, "winter"), (q.turnus.not_summer, "summer"), (q.turnus.not_irregular, "irregular")]
                .iter()
                .filter(|(on, _)| *on)
                .map(|(_, code)| *code)
                .collect();
        if !not_turnus.is_empty() {
            out.push(("not-turnus", join(not_turnus)));
        }
        if let Some(parity) = q.turnus.year_parity {
            out.push(("years", parity.code().to_string()));
        }
        if !q.teaching_forms.is_empty() {
            out.push(("form", join(q.teaching_forms.iter().map(|form| form.code()).collect())));
        }
        if !q.teaching_forms_exclude.is_empty() {
            out.push(("not-form", join(q.teaching_forms_exclude.iter().map(|form| form.code()).collect())));
        }
        if let Some(n) = q.duration_semesters {
            out.push(("duration", n.to_string()));
        }
        let yes_no = |value: bool| if value { "yes" } else { "no" }.to_string();
        if let Some(limited) = q.limited {
            out.push(("limited", yes_no(limited)));
        }
        if let Some(fues) = q.fues {
            out.push(("fues", if fues { "only" } else { "none" }.to_string()));
        }
        let exams: Vec<&str> =
            q.exam_forms.iter().map(|form| form.code()).chain(q.exam_parts.iter().map(|part| part.code())).collect();
        if !exams.is_empty() {
            out.push(("exam", join(exams)));
        }
        if !q.exam_parts_exclude.is_empty() {
            out.push(("not-exam", join(q.exam_parts_exclude.iter().map(|part| part.code()).collect())));
        }
        if let Some(graded) = q.graded {
            out.push(("graded", yes_no(graded)));
        }
        if let Some(offer) = &q.offer {
            if offer.len() == OfferStatus::ALL.len() {
                out.push(("status", "all".to_string()));
            } else if !offer.is_empty() {
                out.push(("status", join(offer.iter().map(|status| status.code()).collect())));
            }
        }
        if let Some(min) = q.credits_min {
            out.push(("ects_min", min.to_string()));
        }
        if let Some(max) = q.credits_max {
            out.push(("ects_max", max.to_string()));
        }
        if !q.campuses.is_empty() {
            out.push(("campus", join(q.campuses.iter().map(|campus| campus.code()).collect())));
        }
        if !q.campuses_exclude.is_empty() {
            out.push(("not-campus", join(q.campuses_exclude.iter().map(|campus| campus.code()).collect())));
        }
        if !q.languages.is_empty() {
            out.push(("lang", join(q.languages.iter().map(|language| language.code()).collect())));
        }
        if !q.languages_exclude.is_empty() {
            out.push(("not-lang", join(q.languages_exclude.iter().map(|language| language.code()).collect())));
        }
        if let Some(marked) = q.marked {
            out.push(("marked", if marked { "only" } else { "none" }.to_string()));
        }
        if q.prerequisites_met_by.is_some() {
            out.push(("prereqs", "met".to_string()));
        }
        match q.sort {
            SortKey::Default => {}
            SortKey::Title => out.push(("sort", "title".to_string())),
            SortKey::Id => out.push(("sort", "id".to_string())),
            SortKey::Credits => out.push(("sort", "ects".to_string())),
            SortKey::Events => out.push(("sort", "events".to_string())),
        }
        if q.descending {
            out.push(("desc", "1".to_string()));
        }
        if self.page > 1 {
            out.push(("page", self.page.to_string()));
        }
        if let Some(id) = &self.open {
            out.push(("open", id.clone()));
        }

        out.iter().map(|(key, value)| format!("{key}={}", encode(value))).collect::<Vec<_>>().join("&")
    }

    /// `/catalog` with the canonical query string.
    pub fn path(&self) -> String {
        let query = self.to_query_string();
        if query.is_empty() {
            CATALOG.to_string()
        } else {
            format!("{CATALOG}?{query}")
        }
    }

    /// The same filter on another page.
    pub fn with_page(&self, page: u64) -> Self {
        Self { query: self.query.clone(), page, open: self.open.clone() }
    }

    /// The same list with this module's preview open, or (`None`) with the preview closed.
    pub fn with_open(&self, id: Option<&str>) -> Self {
        Self { query: self.query.clone(), page: self.page, open: id.map(str::to_string) }
    }
}

impl ExamPart {
    pub fn code(self) -> &'static str {
        match self {
            ExamPart::Written => "written",
            ExamPart::Oral => "oral",
            ExamPart::Paper => "paper",
            ExamPart::Presentation => "presentation",
            ExamPart::Project => "project",
            ExamPart::Practical => "practical",
        }
    }
}

/// Splits and decodes a query string. Pairs without `=` get an empty value.
pub fn parse_pairs(raw_query: &str) -> Vec<(String, String)> {
    raw_query
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

/// Percent-encodes everything except unreserved characters and `,`; a space becomes `+`.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => out.push(byte as char),
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Decodes `%XX` and `+`. Broken escapes stay as they are; invalid UTF-8 is replaced.
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        let escaped = if byte == b'%' {
            bytes.get(i + 1..i + 3).and_then(|hex| std::str::from_utf8(hex).ok()).and_then(|hex| u8::from_str_radix(hex, 16).ok())
        } else {
            None
        };
        match (byte, escaped) {
            (_, Some(value)) => {
                out.push(value);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (other, _) => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_catalog_has_no_query_string() {
        assert_eq!(CatalogUrl::default().path(), "/catalog");
        assert_eq!(CatalogUrl::parse(""), CatalogUrl::default());
        assert_eq!(CatalogUrl::parse("?"), CatalogUrl::default());
    }

    #[test]
    fn a_full_filter_survives_the_round_trip() {
        let url = CatalogUrl {
            query: CatalogQuery {
                text: "Lineare Algebra & Ökologie".into(),
                program: Some(ProgramScope {
                    program_slug: "bachelor-informatik-2008".into(),
                    relation: ProgramRelation::Fues,
                    plan_semester: Some(PlanSemesterFilter::Semester(3)),
                    kinds: vec![KindFilter::Stated(ModuleKind::Elective), KindFilter::Unstated],
                    kinds_exclude: vec![KindFilter::Stated(ModuleKind::Thesis)],
                    area: Some(17),
                    semester_areas: Vec::new(),
                    semester_electives: false,
                }),
                lecturers_include: vec!["Köhler, Ekkehard".into()],
                lecturers_exclude: vec!["Meer, Klaus".into(), "Wachsmuth, Gerd".into()],
                department_id: Some(7),
                turnus: TurnusFilter {
                    winter: true,
                    irregular: true,
                    not_summer: true,
                    year_parity: Some(TurnusParity::Odd),
                    ..Default::default()
                },
                teaching_forms: vec![TeachingForm::Lecture, TeachingForm::Exercise],
                teaching_forms_exclude: vec![TeachingForm::Seminar],
                duration_semesters: Some(2),
                limited: Some(false),
                fues: Some(true),
                exam_forms: vec![ExamForm::Mca],
                exam_parts: vec![ExamPart::Oral],
                exam_parts_exclude: vec![ExamPart::Presentation],
                graded: Some(true),
                offer: Some(OfferStatus::ALL.to_vec()),
                credits_min: Some(5.0),
                credits_max: Some(7.5),
                campuses: vec![Campus::Senftenberg],
                campuses_exclude: vec![Campus::Sachsendorf],
                languages: vec![Language::English],
                languages_exclude: vec![Language::German],
                only_ids: None,
                without_ids: Vec::new(),
                marked: Some(true),
                prerequisites_met_by: Some(vec![]),
                sort: SortKey::Credits,
                descending: true,
            },
            page: 4,
            open: Some("12104".into()),
        };
        let text = url.to_query_string();
        assert_eq!(
            text,
            "q=Lineare+Algebra+%26+%C3%96kologie&program=bachelor-informatik-2008&list=fues&semester=3&area=17&kind=elective,none\
             &not-kind=thesis&lecturer=K%C3%B6hler,+Ekkehard&not-lecturer=Meer,+Klaus&not-lecturer=Wachsmuth,+Gerd&department=7\
             &turnus=winter,irregular&not-turnus=summer&years=odd&form=lecture,exercise&not-form=seminar&duration=2&limited=no\
             &fues=only&exam=mca,oral&not-exam=presentation&graded=yes&status=all&ects_min=5&ects_max=7.5\
             &campus=senftenberg&not-campus=sachsendorf&lang=en&not-lang=de&marked=only&prereqs=met&sort=ects&desc=1&page=4&open=12104"
        );
        assert_eq!(CatalogUrl::parse(&text), url);
    }

    #[test]
    fn a_plain_html_form_is_understood() {
        // Repeated checkboxes, empty text inputs, %20 instead of +.
        let url = CatalogUrl::parse("q=&form=lecture&form=exercise&turnus=winter&ects_min=&ects_max=6&lang=de&status=&q=algebra%20I");
        assert_eq!(url.query.text, "algebra I");
        assert_eq!(url.query.teaching_forms, vec![TeachingForm::Lecture, TeachingForm::Exercise]);
        assert!(url.query.turnus.winter && !url.query.turnus.summer);
        assert_eq!((url.query.credits_min, url.query.credits_max), (None, Some(6.0)));
        assert_eq!(url.query.offer, None);
        assert_eq!(url.to_query_string(), "q=algebra+I&turnus=winter&form=lecture,exercise&ects_max=6&lang=de");
    }

    #[test]
    fn included_wins_over_excluded() {
        let url = CatalogUrl::parse("exam=oral&not-exam=oral,presentation&turnus=winter&not-turnus=winter,summer&lang=en&not-lang=en");
        assert_eq!((url.query.exam_parts.clone(), url.query.exam_parts_exclude.clone()), (vec![ExamPart::Oral], vec![ExamPart::Presentation]));
        assert!(url.query.turnus.winter && !url.query.turnus.not_winter && url.query.turnus.not_summer);
        assert_eq!(url.to_query_string(), "turnus=winter&not-turnus=summer&exam=oral&not-exam=presentation&lang=en");
    }

    #[test]
    fn nonsense_is_ignored_not_an_error() {
        let url = CatalogUrl::parse("form=yoga,lecture,lecture&kind=compulsory&semester=99&area=3&page=-3&sort=random&%ZZ=1&=&&graded=maybe&ects_min=NaN");
        assert_eq!(url.query.teaching_forms, vec![TeachingForm::Lecture]);
        assert_eq!(url.query.program, None, "kind, semester and area mean nothing without a program");
        assert_eq!(CatalogUrl::parse("program=x&area=-3").query.program.and_then(|scope| scope.area), None);
        assert_eq!(CatalogUrl::parse("program=x&area=12").path(), "/catalog?program=x&area=12");
        assert_eq!((url.page, url.query.sort, url.query.graded, url.query.credits_min), (1, SortKey::Default, None, None));
        assert_eq!(url.to_query_string(), "form=lecture");
    }

    #[test]
    fn the_marked_modules_have_a_canonical_url() {
        assert_eq!(BookmarksUrl::parse("").path(), "/bookmarks");
        let url = BookmarksUrl::parse("open=11101&desc=1&sort=ECTS&turnus=winter&ids=1,2,3&utm=x");
        assert_eq!((url.season, url.sort, url.descending, url.open.as_deref()), (Some(Season::Winter), BookmarkSort::Credits, true, Some("11101")));
        assert_eq!(url.path(), "/bookmarks?turnus=winter&sort=ects&desc=1&open=11101");
        assert_eq!(BookmarksUrl::parse(url.path().split_once('?').map(|(_, query)| query).unwrap_or_default()), url);
        assert_eq!(url.with_open(None).path(), "/bookmarks?turnus=winter&sort=ects&desc=1");
        // The order of marking has one direction, and nonsense is ignored.
        assert_eq!(BookmarksUrl::parse("sort=added&desc=1").path(), "/bookmarks");
        assert_eq!(BookmarksUrl::parse("sort=random&turnus=spring&open=../../etc&desc=1"), BookmarksUrl::default());
        assert!(is_module_id("11101") && is_module_id("FÜS-1".replace('Ü', "U").as_str()));
        assert!(!is_module_id("") && !is_module_id("1 OR 1=1") && !is_module_id(&"9".repeat(33)) && !is_module_id("a\tb"));
    }

    #[test]
    fn the_program_overview_has_a_canonical_url() {
        assert_eq!(ProgramsUrl::parse("").path(), "/programs");
        let url = ProgramsUrl::parse("form=dual&level=master,bachelor,yoga&plan=1&q=+Informatik+&utm=x");
        assert_eq!((url.levels.clone(), url.forms.clone(), url.with_plan), (vec![LevelGroup::Bachelor, LevelGroup::Master], vec![FormGroup::Dual], true));
        assert_eq!(url.path(), "/programs?q=Informatik&level=bachelor,master&form=dual&plan=1");
        assert_eq!(ProgramsUrl::parse(url.path().split_once('?').map(|(_, query)| query).unwrap_or_default()), url);
        assert!(!ProgramsUrl::parse("plan=0&level=").is_filtered());
    }

    #[test]
    fn paths() {
        assert_eq!(module_path("11101"), "/catalog/module/11101");
        assert_eq!(program_path("bachelor-informatik-2008", ProgramTab::Areas), "/programs/bachelor-informatik-2008/areas");
        assert_eq!(ProgramTab::from_segment("modules"), Some(ProgramTab::Modules));
        assert_eq!(ProgramTab::from_segment("electives"), None);
        let program = ProgramUrl::parse("bachelor-elektrotechnik-2022", ProgramTab::Plan, "variant=2&open=11101&utm=x");
        assert_eq!(program.path(), "/programs/bachelor-elektrotechnik-2022/plan?variant=2&open=11101");
        // An area beside the page, and a module opened out of it keeps it.
        let areas = ProgramUrl::parse("x", ProgramTab::Areas, "area=12&open=11101");
        assert_eq!(areas.query(), "?area=12&open=11101");
        assert_eq!(areas.with_open(None).query(), "?area=12");
        assert_eq!(areas.with_area(None).query(), "");
        assert_eq!(ProgramUrl::parse("x", ProgramTab::Areas, "area=-3").area, None);
        // A row of the plan, and what replaces what.
        let plan = ProgramUrl::parse("x", ProgramTab::Plan, "variant=2&req=7");
        assert_eq!(plan.query(), "?variant=2&req=7");
        assert_eq!(plan.with_area(Some(3)).query(), "?variant=2&area=3");
        assert_eq!(plan.with_open(Some("11101")).query(), "?variant=2&req=7&open=11101");
        // The module fills the page: only with a module, and beside the page again with `with_open`.
        let full = ProgramUrl::parse("x", ProgramTab::Areas, "area=12&open=11101&full=1");
        assert!(full.full);
        assert_eq!(full.query(), "?area=12&open=11101&full=1");
        assert_eq!(full.with_open(Some("11101")).query(), "?area=12&open=11101");
        assert_eq!(full.with_open(None).query(), "?area=12");
        assert_eq!(areas.with_full(true).query(), "?area=12&open=11101&full=1");
        assert!(!ProgramUrl::parse("x", ProgramTab::Plan, "full=1").full);
        assert!(!ProgramUrl::parse("x", ProgramTab::Plan, "open=11101&full=yes").full);
        assert_eq!(ProgramUrl::parse("x", ProgramTab::Plan, "req=0").req, None);
        assert_eq!(program.with_open(None).with_variant(1).path(), "/programs/bachelor-elektrotechnik-2022/plan");
        assert_eq!(ProgramUrl::parse("x", ProgramTab::Plan, "variant=0").variant, 1);
        assert_eq!(ProgramUrl::parse("x", ProgramTab::Plan, "variant=999").variant, 1);
        assert_eq!(ProgramUrl::parse("x", ProgramTab::Plan, "open=../../etc").open, None);
        assert_eq!(ProgramUrl::parse("x", ProgramTab::Areas, "").query(), "");
        assert_eq!(CatalogUrl { page: 3, ..Default::default() }.offset(), 100);
        assert_eq!(CatalogUrl::parse("open=12104").with_open(None).path(), "/catalog");
        assert_eq!(CatalogUrl::parse("open=../../etc").open, None);
        assert_eq!(CatalogUrl::parse("turnus=winter").with_open(Some("11101")).path(), "/catalog?turnus=winter&open=11101");
    }
}
