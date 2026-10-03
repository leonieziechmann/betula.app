//! URLs of the app, in one place for the server, the pages and the tests.
//!
//! `/`                                  landing page
//! `/catalog?…`                         module catalog; the query string is a `CatalogQuery`.
//!                                      `fits=<semester>` (with `fits-skip`, `fits-undated`) is the
//!                                      switch „Passt in meinen Stundenplan"; which modules fit
//!                                      comes from the browser, like the marked ones. `fill=p<n>`
//!                                      (the placeholder a module found there would fill) is the
//!                                      app's: the server's page and its cache key drop it
//! `/catalog/module/<id>`               module page; `?plan=<semester>&fill=p<n>` is the app's hint
//!                                      of where its plan button plans to (`ModuleHint`)
//! `/bookmarks?…[&open=<id>][&full=1]`  the visitor's marked modules (`BookmarksUrl`); which ones
//!                                      they are is never part of a URL, only how they are shown,
//!                                      which module stands beside them, and whether that module
//!                                      fills the page
//! `/programs`                          program overview
//! `/programs/<slug>[/plan|areas|my-plan][?variant=<n>][&open=<id>][&full=1]`   program page, its
//!                                      tabs, which of several study plans is shown, which module
//!                                      stands beside it, and whether that module fills the page
//!                                      (`ProgramUrl`)
//! `/studyplan?sem=…&view=…&open=<id>&row=<key>&import=…&variant=<n>[&share=<code>]`   the
//!                                      visitor's Studienplan (`StudyplanUrl`): which semester and
//!                                      view, the module and Termin beside it, the Regelstudienplan
//!                                      being taken over. What is planned lives in the browser,
//!                                      never in a URL, but for a plan handed on by a link
//!                                      (`share`, `timetable::share`), which the page offers to
//!                                      take over and whose link preview names its modules
//! `/calendar/<code>.ics`               a calendar subscription, served by the server (not a page):
//!                                      the code says semester, modules and what is hidden
//!                                      (`timetable::subscription`)
//!
//! `open` and `full` mean the same on every page that has them (`LocalView`): the module shown
//! in place, beside the page or filling it, without leaving the page's area.
//!
//! Most filters can also exclude: `exam=written&not-exam=presentation` lists modules with a
//! written exam and without a presentation. A value that is both included and excluded counts
//! as included.
//!
//! The catalog parameters are readable and tolerant: a plain HTML form (no JavaScript) sends
//! repeated parameters and empty values, the app writes the canonical form, a hand-edited
//! link may contain nonsense. Unknown names and values are ignored, never an error.

use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::SemesterKey;
use folia_calendar::share::{self, SharedPlan};
use folia_locale::Locale;
use folia_model::labels::{Campus, ExamForm, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity};
use serde::{Deserialize, Serialize};

use crate::filter::{
    CatalogQuery, ExamPart, FitsFilter, KindFilter, Language, PlanSemesterFilter, ProgramRelation, ProgramScope,
    SortKey, TurnusFilter,
};

pub use folia_model::ids::{is_module_id, is_program_id};

pub const HOME: &str = "/";
pub const CATALOG: &str = "/catalog";
pub const PROGRAMS: &str = "/programs";
/// „Merkliste": the modules the visitor has marked. The list itself lives in the browser.
pub const BOOKMARKS: &str = "/bookmarks";
/// The legal pages, under the German names people look for.
pub const IMPRINT: &str = "/impressum";
pub const PRIVACY: &str = "/datenschutz";
/// „Studienplan": the visitor's modules per calendar semester. The plan itself lives in the
/// browser; the address says only how it is shown (`StudyplanUrl`).
pub const STUDYPLAN: &str = "/studyplan";

/// The Stundenplan's address that hands the plan of `code` on (`share::SharedPlan`).
pub fn share_path(code: &str) -> String {
    format!("{STUDYPLAN}?{}={code}", share::PARAM)
}

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
    /// „Mein Plan": the visitor's plan of the whole study, semester by semester. A placeholder
    /// for now (owner, 2026-09-25); it took the place of „Alle Module", whose modules are the
    /// catalog of the program (`program_catalog_path`).
    MyPlan,
}

impl ProgramTab {
    pub const ALL: &'static [Self] = &[Self::Plan, Self::Areas, Self::MyPlan];

    pub fn segment(self) -> &'static str {
        match self {
            ProgramTab::Plan => "plan",
            ProgramTab::Areas => "areas",
            ProgramTab::MyPlan => "my-plan",
        }
    }

    pub fn from_segment(segment: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|tab| tab.segment() == segment)
    }

    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, ProgramTab::Plan) => "Regelstudienplan",
            (Locale::De, ProgramTab::Areas) => "Wahlpflicht & Bereiche",
            (Locale::De, ProgramTab::MyPlan) => "Mein Plan",
            (Locale::En, ProgramTab::Plan) => "Standard study plan",
            (Locale::En, ProgramTab::Areas) => "Electives & areas",
            (Locale::En, ProgramTab::MyPlan) => "My plan",
        }
    }

    /// Whether a search engine is meant to find the tab (the sitemap, `noindex`): „Mein Plan"
    /// is the visitor's, and a placeholder so far.
    pub fn indexed(self) -> bool {
        self != ProgramTab::MyPlan
    }
}

pub fn program_path(slug: &str, tab: ProgramTab) -> String {
    format!("/programs/{}/{}", encode(slug), tab.segment())
}

/// The catalog narrowed down to the modules of a program (its curriculum): where „Alle Module"
/// of the program's page went.
pub fn program_catalog_path(slug: &str, open: Option<&str>) -> String {
    let query = CatalogQuery { program: Some(ProgramScope { program_slug: slug.to_string(), ..Default::default() }), ..Default::default() };
    CatalogUrl { query, page: 1, open: open.map(str::to_string), fill: None }.path()
}

/// Whether `address` (a path of the app with its query, without the language's prefix) is a page
/// that search engines list, and not a view of one: a filter, an order or a search of a list, a
/// page of a filtered list, what the app lays beside a page or fills it with. Of the addresses
/// with a query only two are pages, each as its canonical address writes it: a further page of
/// the unfiltered catalog (`/catalog?page=<n>`; `page` comes after every filter, so
/// `/catalog?turnus=winter&page=2` is a view) and the plan of a further study direction
/// (`/programs/<slug>/plan?variant=<n>`). What is the visitor's own and lives in their browser is
/// no page for search engines either: the Merkliste, the Stundenplan and a program's „Mein Plan"
/// (`ProgramTab::indexed`). An older examination regulation says `noindex` by its data, not by
/// its address, and its links are followed: here it is a page.
///
/// A link to what is not listed carries `rel="nofollow"` (`app::seo::nofollow`), robots.txt keeps
/// crawlers out of the views of the lists (`server::api::robots`), and the page cache lets views
/// go first (`server::cache`).
pub fn listed(address: &str) -> bool {
    let number = |value: &str| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit());
    let address = address.split('#').next().unwrap_or_default();
    let (path, query) = match address.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (address, None),
    };
    let program_tab = path.strip_prefix("/programs/").and_then(|rest| rest.split_once('/')).and_then(|(_, tab)| ProgramTab::from_segment(tab));
    if path == BOOKMARKS || path == STUDYPLAN || program_tab.is_some_and(|tab| !tab.indexed()) {
        return false;
    }
    match query {
        None => true,
        Some(query) if path == CATALOG => query.strip_prefix("page=").is_some_and(number),
        Some(query) if program_tab == Some(ProgramTab::Plan) => query.strip_prefix("variant=").is_some_and(number),
        Some(_) => false,
    }
}

/// How many study plans of one program can be told apart in the URL. A program has one plan per
/// study direction; the highest seen so far is eight.
pub const MAX_PLAN_VARIANTS: usize = 20;
/// How many rows of one study plan can be told apart in the URL (the longest has 29).
pub const MAX_PLAN_ROWS: usize = 200;

/// The address of a page that shows the modules it lists in place, as a *local view* (owner,
/// 2026-09-24: „als lokale Ansicht in jedem Tab"): a module opened there stands beside the page
/// (`open=<id>`), and „Vollbild" lets it fill the page with the module's whole page
/// (`open=<id>&full=1`). Either way the address stays the page's, so the area, its tab, the
/// history and „Zurück" stay what they were. The program page and the marked modules are such
/// pages (the semester plan will be one). The catalog is not: its modules have their own page
/// (`module_path`), which is in the catalog's area anyway.
///
/// The two parameters are read and written the same way wherever they are (`local_from_pairs`,
/// `local_pairs`); the browser app does the rest for every such page (`app::local`).
pub trait LocalView: Clone {
    /// The module beside the page, or filling it (`open`).
    fn open(&self) -> Option<&str>;
    /// That module fills the page (`full=1`). Never without `open`.
    fn full(&self) -> bool;
    /// The same page with `open` beside it, or filling it (`full`, only with a module).
    fn with_module(&self, open: Option<&str>, full: bool) -> Self;
    /// The address, as links write it.
    fn path(&self) -> String;

    /// The same page with this module beside it, or (`None`) with none. Beside it, not filling
    /// it: what fills the page is asked for with `with_full`.
    fn with_open(&self, id: Option<&str>) -> Self {
        self.with_module(id, false)
    }

    /// The module beside the page fills it (`true`), or stands beside it again (`false`).
    fn with_full(&self, full: bool) -> Self {
        self.with_module(self.open(), full)
    }
}

/// `open` and `full` of a local view (`LocalView`), from the pairs of a query: an `open` that is
/// no module id is not there, and `full` is nothing without `open`, nor anything but `full=1`.
pub fn local_from_pairs(pairs: &[(String, String)]) -> (Option<String>, bool) {
    let first = |name: &str| pairs.iter().find(|(key, _)| key == name).map(|(_, value)| value.trim());
    let open = first("open").filter(|id| is_module_id(id)).map(str::to_string);
    let full = open.is_some() && first("full") == Some("1");
    (open, full)
}

/// The same as pairs of a query, not yet encoded, in the order they end an address: `open`, then
/// `full`.
pub fn local_pairs(open: Option<&str>, full: bool) -> Vec<(&'static str, String)> {
    let Some(id) = open else { return Vec::new() };
    let mut out = vec![("open", id.to_string())];
    if full {
        out.push(("full", "1".to_string()));
    }
    out
}

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
    /// program's area, so that „Vollbild" neither changes the area nor the tab (a local view,
    /// `LocalView`). Nothing without `open`.
    pub full: bool,
}

impl ProgramUrl {
    pub fn new(slug: &str, tab: ProgramTab) -> Self {
        Self { slug: slug.to_string(), tab, variant: 1, open: None, area: None, req: None, full: false }
    }

    pub fn parse(slug: &str, tab: ProgramTab, raw_query: &str) -> Self {
        let pairs = parse_pairs(raw_query);
        let first = |name: &str| pairs.iter().find(|(key, _)| key == name).map(|(_, value)| value.trim().to_string());
        let (open, full) = local_from_pairs(&pairs);
        Self {
            slug: slug.to_string(),
            tab,
            variant: first("variant").and_then(|value| value.parse::<usize>().ok()).filter(|n| (1..=MAX_PLAN_VARIANTS).contains(n)).unwrap_or(1),
            open,
            full,
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
        out.extend(local_pairs(self.open.as_deref(), self.full).into_iter().map(|(key, value)| format!("{key}={}", encode(&value))));
        match out.is_empty() {
            true => String::new(),
            false => format!("?{}", out.join("&")),
        }
    }

    pub fn path(&self) -> String {
        format!("{}{}", program_path(&self.slug, self.tab), self.query())
    }

    /// The same page with this area beside it; what was shown so far makes way for it.
    pub fn with_area(&self, id: Option<i64>) -> Self {
        Self { area: id, open: None, req: None, full: false, ..self.clone() }
    }

    /// The area opened out of the row of the plan beside the page: the row stays in the address,
    /// so the area's panel says how one got there („Anwendungsfach / Mathematik") and closing it
    /// returns to the row (owner, 2026-09-23). Without a row the same as `with_area`.
    pub fn with_area_keeping_req(&self, id: i64) -> Self {
        Self { area: Some(id), open: None, full: false, ..self.clone() }
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

impl LocalView for ProgramUrl {
    fn open(&self) -> Option<&str> {
        self.open.as_deref()
    }

    fn full(&self) -> bool {
        self.full
    }

    /// What else stands beside the page stays: a module opened out of an area keeps it, so
    /// closing the module returns to the area.
    fn with_module(&self, open: Option<&str>, full: bool) -> Self {
        Self { open: open.map(str::to_string), full: full && open.is_some(), ..self.clone() }
    }

    fn path(&self) -> String {
        ProgramUrl::path(self)
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

    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (_, LevelGroup::Bachelor) => "Bachelor",
            (_, LevelGroup::Master) => "Master",
            (Locale::De, LevelGroup::Teaching) => "Lehramt",
            (Locale::De, LevelGroup::Doctoral) => "Promotion",
            (Locale::De, LevelGroup::Other) => "Sonstige",
            (Locale::En, LevelGroup::Teaching) => "Teacher training",
            (Locale::En, LevelGroup::Doctoral) => "Doctorate",
            (Locale::En, LevelGroup::Other) => "Other",
        }
    }

    pub fn of(level: &folia_model::labels::Code<folia_model::labels::DegreeLevel>) -> Self {
        use folia_model::labels::DegreeLevel;
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

    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (_, FormGroup::Dual) => "Dual",
            (Locale::De, FormGroup::DoubleDegree) => "Doppelabschluss",
            (Locale::De, FormGroup::Flexible) => "Teilzeit & Fern",
            (Locale::En, FormGroup::DoubleDegree) => "Double degree",
            (Locale::En, FormGroup::Flexible) => "Part-time & distance",
        }
    }

    pub fn of(variant: Option<&folia_model::labels::Code<folia_model::labels::StudyVariant>>) -> Option<Self> {
        use folia_model::labels::StudyVariant;
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

/// How the list of marked modules is ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, BookmarkSort::Added) => "Zuletzt gemerkt",
            (Locale::De, BookmarkSort::Title) => "Titel",
            (Locale::De, BookmarkSort::Credits) => "Leistungspunkte",
            (Locale::De, BookmarkSort::Events) => "Termine",
            (Locale::En, BookmarkSort::Added) => "Recently saved",
            (Locale::En, BookmarkSort::Title) => "Title",
            (Locale::En, BookmarkSort::Credits) => "Credit points",
            (Locale::En, BookmarkSort::Events) => "Dates",
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

    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (_, Season::Winter) => "Winter",
            (Locale::De, Season::Summer) => "Sommer",
            (Locale::En, Season::Summer) => "Summer",
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
    /// The module of `open` fills the page (`full=1`): its whole page, shown in the list's place,
    /// so that „Vollbild" stays among the marked modules (a local view, `LocalView`), as it does
    /// on a program's page. Nothing without `open`.
    pub full: bool,
}

impl BookmarksUrl {
    pub fn parse(raw_query: &str) -> Self {
        let pairs = parse_pairs(raw_query);
        let first = |name: &str| pairs.iter().find(|(key, value)| key == name && !value.trim().is_empty()).map(|(_, value)| value.trim().to_ascii_lowercase());
        let sort = first("sort").and_then(|code| BookmarkSort::ALL.iter().copied().find(|sort| sort.code() == code)).unwrap_or_default();
        let (open, full) = local_from_pairs(&pairs);
        Self {
            season: first("turnus").and_then(|code| Season::ALL.iter().copied().find(|season| season.code() == code)),
            sort,
            descending: sort != BookmarkSort::Added && first("desc").is_some(),
            open,
            full,
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
        out.extend(local_pairs(self.open.as_deref(), self.full));
        if out.is_empty() {
            return BOOKMARKS.to_string();
        }
        format!("{BOOKMARKS}?{}", out.iter().map(|(key, value)| format!("{key}={}", encode(value))).collect::<Vec<_>>().join("&"))
    }
}

impl LocalView for BookmarksUrl {
    fn open(&self) -> Option<&str> {
        self.open.as_deref()
    }

    fn full(&self) -> bool {
        self.full
    }

    fn with_module(&self, open: Option<&str>, full: bool) -> Self {
        Self { open: open.map(str::to_string), full: full && open.is_some(), ..self.clone() }
    }

    fn path(&self) -> String {
        BookmarksUrl::path(self)
    }
}

/// A view of the Studienplan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PlanView {
    /// „Woche": the Regelwoche, the recurring Termine of one semester as a timetable.
    #[default]
    Week,
    /// „Termine": every date of the semester, week by week.
    Dates,
    /// „Prüfungen": the exam sittings of the semester's planned modules.
    Exams,
    /// „Übersicht": every semester of the plan at a glance.
    Overview,
}

impl PlanView {
    pub const ALL: &'static [Self] = &[Self::Week, Self::Dates, Self::Exams, Self::Overview];

    pub fn code(self) -> &'static str {
        match self {
            PlanView::Week => "week",
            PlanView::Dates => "dates",
            PlanView::Exams => "exams",
            PlanView::Overview => "all",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|view| view.code() == code)
    }

    pub fn label(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::De, PlanView::Week) => "Woche",
            (Locale::De, PlanView::Dates) => "Termine",
            (Locale::De, PlanView::Exams) => "Prüfungen",
            (Locale::De, PlanView::Overview) => "Übersicht",
            (Locale::En, PlanView::Week) => "Week",
            (Locale::En, PlanView::Dates) => "Dates",
            (Locale::En, PlanView::Exams) => "Exams",
            (Locale::En, PlanView::Overview) => "Overview",
        }
    }
}

/// How long a program slug in `import=` may be (the longest of the snapshot has 92 characters).
const MAX_SLUG: usize = 120;

/// `mine` or a program's slug: lowercase ASCII letters, digits and `-`.
fn is_import(value: &str) -> bool {
    (1..=MAX_SLUG).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// What the Studienplan's address says: `/studyplan?sem=2026W&view=dates&open=12104&row=148369-aaf38`.
///
/// How the plan is shown, never what is in it (R20): which semester and which view, the one module
/// and Termin shown beside the plan (as the Merkliste's `open`), and which program's
/// Regelstudienplan is being taken over. Tolerant and canonical like `CatalogUrl`. The server
/// renders one explanation for every query (R9) and caches it by the path alone. The one exception
/// is `share`: a plan somebody handed on by a link (`timetable::share`, the owner's decision of
/// 2026-09-26), whose modules the server's page names in its link preview, so it is part of the
/// page's cache key.
#[derive(Clone, Debug, PartialEq)]
pub struct StudyplanUrl {
    /// The calendar semester shown (`SemesterKey::key()`). `None` is the default semester, which
    /// depends on the plan and the date and so is the page's to work out.
    pub sem: Option<String>,
    pub view: PlanView,
    /// The module shown beside the plan (`open=<id>`).
    pub open: Option<String>,
    /// The Termin of that module the aside points at (`row=<event>-<fp>`, `RowKey::text`).
    /// Nothing without `open`.
    pub row: Option<String>,
    /// The Regelstudienplan being taken over: `mine` (Mein Studiengang) or a program's slug.
    pub import: Option<String>,
    /// Which of that program's study plans, 1-based as on the program's page; the first is not
    /// written. Nothing without `import`.
    pub variant: usize,
    /// The code of a plan handed on by a link (`share=<code>`, `timetable::share::SharedPlan`),
    /// which the page offers to take over. Only a code that decodes.
    pub share: Option<String>,
}

impl Default for StudyplanUrl {
    fn default() -> Self {
        Self { sem: None, view: PlanView::Week, open: None, row: None, import: None, variant: 1, share: None }
    }
}

impl StudyplanUrl {
    pub fn parse(raw_query: &str) -> Self {
        let pairs = parse_pairs(raw_query);
        let first = |name: &str| first_value(&pairs, name);
        let open = first("open").filter(|id| is_module_id(id));
        let import = first("import").map(|value| value.to_ascii_lowercase()).filter(|value| is_import(value));
        // A week (`week=2026-10-14`) is no part of the address: the dated weeks are the view
        // „Termine", which finds the current one by itself. Like every unknown name it is ignored.
        Self {
            sem: first("sem").and_then(|key| SemesterKey::parse(&key)).map(SemesterKey::key),
            view: first("view").and_then(|code| PlanView::from_code(&code.to_ascii_lowercase())).unwrap_or_default(),
            row: open.as_ref().and(first("row")).and_then(|key| RowKey::parse(&key.to_ascii_lowercase())).map(RowKey::text),
            variant: match import {
                Some(_) => first("variant").and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=MAX_PLAN_VARIANTS).contains(n)).unwrap_or(1),
                None => 1,
            },
            open,
            import,
            share: first(share::PARAM).filter(|code| SharedPlan::from_code(code).is_some()),
        }
    }

    /// The semester of `sem`, when it names one.
    pub fn semester(&self) -> Option<SemesterKey> {
        self.sem.as_deref().and_then(SemesterKey::parse)
    }

    /// `/studyplan` with the canonical query string: `sem`, `view`, `open`, `row`, `import`,
    /// `variant`, `share`, each only when it says something.
    pub fn path(&self) -> String {
        let mut out: Vec<(&str, String)> = Vec::new();
        if let Some(sem) = &self.sem {
            out.push(("sem", sem.clone()));
        }
        if self.view != PlanView::Week {
            out.push(("view", self.view.code().to_string()));
        }
        if let Some(id) = &self.open {
            out.push(("open", id.clone()));
            if let Some(row) = &self.row {
                out.push(("row", row.clone()));
            }
        }
        if let Some(import) = &self.import {
            out.push(("import", import.clone()));
            if self.variant > 1 {
                out.push(("variant", self.variant.to_string()));
            }
        }
        if let Some(code) = &self.share {
            out.push((share::PARAM, code.clone()));
        }
        if out.is_empty() {
            return STUDYPLAN.to_string();
        }
        format!("{STUDYPLAN}?{}", out.iter().map(|(key, value)| format!("{key}={}", encode(value))).collect::<Vec<_>>().join("&"))
    }

    /// Another semester (`None`: the default one). The module beside the plan was one of the old
    /// semester's, so it closes.
    pub fn with_semester(&self, sem: Option<SemesterKey>) -> Self {
        Self { sem: sem.map(SemesterKey::key), open: None, row: None, ..self.clone() }
    }

    pub fn with_view(&self, view: PlanView) -> Self {
        Self { view, ..self.clone() }
    }

    /// This module beside the plan, pointing at this Termin of it, or (`None`) nothing beside it.
    pub fn with_open(&self, id: Option<&str>, row: Option<RowKey>) -> Self {
        let open = id.map(str::to_string);
        Self { row: open.as_ref().and(row).map(RowKey::text), open, ..self.clone() }
    }

    /// The same page once the Regelstudienplan is taken over (or the panel closed).
    pub fn without_import(&self) -> Self {
        Self { import: None, variant: 1, ..self.clone() }
    }

    /// The same page once a shared plan is taken over or put aside.
    pub fn without_share(&self) -> Self {
        Self { share: None, ..self.clone() }
    }
}

/// The highest placeholder number (the store's `pid`) a `fill=` may name.
pub const MAX_PID: u32 = 9_999;

/// `p3` → 3: the placeholder of the visitor's plan that `fill=` names.
fn parse_fill(text: &str) -> Option<u32> {
    let text = text.trim();
    let digits = text.strip_prefix('p').or_else(|| text.strip_prefix('P'))?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok().filter(|pid| (1..=MAX_PID).contains(pid))
}

/// What a module's page is asked to plan the module into, when it was reached from the catalog's
/// „Passt in meinen Stundenplan" (`/catalog/module/<id>?plan=2026W&fill=p3`): the semester its
/// plan button aims at and the placeholder the module would fill. The app's alone: the server keys
/// the page by its path and ignores both, like every other query of a module page.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModuleHint {
    /// `SemesterKey::key()`.
    pub plan: Option<String>,
    /// A placeholder's number in the visitor's plan (`fill=p<n>`).
    pub fill: Option<u32>,
}

impl ModuleHint {
    pub fn parse(raw_query: &str) -> Self {
        let pairs = parse_pairs(raw_query);
        Self {
            plan: first_value(&pairs, "plan").and_then(|key| SemesterKey::parse(&key)).map(SemesterKey::key),
            fill: first_value(&pairs, "fill").and_then(|text| parse_fill(&text)),
        }
    }

    /// `?plan=2026W&fill=p3`, what follows `module_path(id)`; empty without a hint.
    pub fn query(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        if let Some(plan) = &self.plan {
            out.push(format!("plan={}", encode(plan)));
        }
        if let Some(pid) = self.fill {
            out.push(format!("fill=p{pid}"));
        }
        match out.is_empty() {
            true => String::new(),
            false => format!("?{}", out.join("&")),
        }
    }
}

/// How many modules one catalog page lists.
pub const PAGE_SIZE: u64 = 50;

/// How many areas one address may name (`area=12,7`): a row of the plan means at most a handful.
pub const MAX_AREAS: usize = 20;

/// What a catalog URL says: the filter and the page (1-based).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CatalogUrl {
    pub query: CatalogQuery,
    pub page: u64,
    /// The module previewed next to the list (`open=<id>`). Its own page is `module_path`.
    pub open: Option<String>,
    /// The placeholder of the visitor's Studienplan that a module found here fills (`fill=p3`):
    /// „Modul finden" of a plan row leads here, and the preview's plan button then plans into it.
    /// The app's, like `open`: the server's page drops it and its cache key ignores it.
    pub fill: Option<u32>,
}

impl Default for CatalogUrl {
    fn default() -> Self {
        Self { query: CatalogQuery::default(), page: 1, open: None, fill: None }
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
                areas: {
                    let mut areas: Vec<i64> = Vec::new();
                    for id in codes("area").iter().filter_map(|value| value.parse::<i64>().ok()).filter(|id| *id > 0) {
                        if !areas.contains(&id) && areas.len() < MAX_AREAS {
                            areas.push(id);
                        }
                    }
                    areas
                },
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
            // Derived by the page, never read from a URL.
            text_resolution: None,
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
            scheduled: yes_no("events"),
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
            // The same for the modules that fit the plan: the URL says the semester and what is
            // compared, the browser works out which modules they are.
            fits: first("fits").and_then(|key| SemesterKey::parse(&key)).map(|key| {
                let skipped = codes("fits-skip");
                let compared = |code: &str| !skipped.iter().any(|skip| skip == code);
                let [lecture, exercise, exam] = FitsFilter::CLASSES;
                FitsFilter {
                    semester: key.key(),
                    lectures: compared(lecture),
                    exercises: compared(exercise),
                    exams: compared(exam),
                    undated: first("fits-undated").as_deref() == Some("1"),
                }
            }),
            fits_ids: None,
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
        let fill = first("fill").and_then(|text| parse_fill(&text));
        Self { query, page, open, fill }
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
            if !scope.areas.is_empty() {
                out.push(("area", scope.areas.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",")));
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
        if let Some(scheduled) = q.scheduled {
            out.push(("events", yes_no(scheduled)));
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
        if let Some(fits) = &q.fits {
            out.push(("fits", fits.semester.clone()));
            let skipped = fits.skipped();
            if !skipped.is_empty() {
                out.push(("fits-skip", join(skipped)));
            }
            if fits.undated {
                out.push(("fits-undated", "1".to_string()));
            }
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
        if let Some(pid) = self.fill {
            out.push(("fill", format!("p{pid}")));
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
        Self { page, ..self.clone() }
    }

    /// The same list with this module's preview open, or (`None`) with the preview closed.
    pub fn with_open(&self, id: Option<&str>) -> Self {
        Self { open: id.map(str::to_string), ..self.clone() }
    }

    /// The same list looking for a module for this placeholder of the plan, or (`None`) for none.
    pub fn with_fill(&self, fill: Option<u32>) -> Self {
        Self { fill, ..self.clone() }
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

/// The first non-empty value of a name, trimmed: a plain form sends empty inputs too.
fn first_value(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs.iter().find(|(key, value)| key == name && !value.trim().is_empty()).map(|(_, value)| value.trim().to_string())
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
                text_resolution: None,
                program: Some(ProgramScope {
                    program_slug: "bachelor-informatik-2008".into(),
                    relation: ProgramRelation::Fues,
                    plan_semester: Some(PlanSemesterFilter::Semester(3)),
                    kinds: vec![KindFilter::Stated(ModuleKind::Elective), KindFilter::Unstated],
                    kinds_exclude: vec![KindFilter::Stated(ModuleKind::Thesis)],
                    areas: vec![17],
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
                scheduled: Some(true),
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
                fits: Some(FitsFilter { semester: "2026W".into(), lectures: true, exercises: false, exams: true, undated: true }),
                fits_ids: None,
            },
            page: 4,
            open: Some("12104".into()),
            fill: Some(3),
        };
        let text = url.to_query_string();
        assert_eq!(
            text,
            "q=Lineare+Algebra+%26+%C3%96kologie&program=bachelor-informatik-2008&list=fues&semester=3&area=17&kind=elective,none\
             &not-kind=thesis&lecturer=K%C3%B6hler,+Ekkehard&not-lecturer=Meer,+Klaus&not-lecturer=Wachsmuth,+Gerd&department=7\
             &turnus=winter,irregular&not-turnus=summer&years=odd&form=lecture,exercise&not-form=seminar&duration=2&limited=no\
             &fues=only&exam=mca,oral&not-exam=presentation&graded=yes&events=yes&status=all&ects_min=5&ects_max=7.5\
             &campus=senftenberg&not-campus=sachsendorf&lang=en&not-lang=de&marked=only&fits=2026W&fits-skip=exercise&fits-undated=1\
             &prereqs=met&sort=ects&desc=1&page=4&open=12104&fill=p3"
        );
        assert_eq!(CatalogUrl::parse(&text), url);
    }

    #[test]
    fn the_fit_switch_is_read_tolerantly() {
        // The semester in either case, skipped classes repeated or unknown, the switch's
        // companions without the switch: all as a plain form or a hand-edited link sends them.
        let url = CatalogUrl::parse("fits=2026w&fits-skip=EXAM,yoga&fits-skip=lecture,exam&fits-undated=1");
        assert_eq!(url.query.fits, Some(FitsFilter { semester: "2026W".into(), lectures: false, exercises: true, exams: false, undated: true }));
        assert_eq!(url.to_query_string(), "fits=2026W&fits-skip=lecture,exam&fits-undated=1");
        assert_eq!(CatalogUrl::parse("fits=2026W").query.fits, Some(FitsFilter::all("2026W")));
        assert_eq!(CatalogUrl::parse("fits=2026W").to_query_string(), "fits=2026W");
        for nonsense in ["fits=2026X", "fits=1999W", "fits=", "fits-skip=exam&fits-undated=1", "fits=2026W2"] {
            assert_eq!(CatalogUrl::parse(nonsense).query.fits, None, "{nonsense}");
        }
        assert_eq!(CatalogUrl::parse("fits=2026S&fits-undated=yes").query.fits.map(|fits| fits.undated), Some(false));
        // The ids never come from an address.
        assert_eq!(CatalogUrl::parse("fits=2026W&fits_ids=11103&fits-ids=11103").query.fits_ids, None);
    }

    #[test]
    fn a_placeholder_to_fill_travels_with_the_list_in_the_app() {
        let url = CatalogUrl::parse("fits=2026W&fill=p3&turnus=winter");
        assert_eq!(url.fill, Some(3));
        assert_eq!(url.path(), "/catalog?turnus=winter&fits=2026W&fill=p3");
        assert_eq!(url.with_page(2).path(), "/catalog?turnus=winter&fits=2026W&page=2&fill=p3");
        assert_eq!(url.with_open(Some("11103")).path(), "/catalog?turnus=winter&fits=2026W&open=11103&fill=p3");
        assert_eq!(url.with_fill(None).path(), "/catalog?turnus=winter&fits=2026W");
        assert_eq!(CatalogUrl::default().with_fill(Some(12)).path(), "/catalog?fill=p12");
        assert_eq!(CatalogUrl::parse("fill=P07").fill, Some(7));
        for nonsense in ["fill=3", "fill=p0", "fill=p10000", "fill=p-1", "fill=p+3", "fill=p", "fill=px", "fill=p99999999999999999999"] {
            assert_eq!(CatalogUrl::parse(nonsense).fill, None, "{nonsense}");
        }
    }

    #[test]
    fn the_studyplan_has_a_canonical_url() {
        assert_eq!(StudyplanUrl::parse(""), StudyplanUrl::default());
        assert_eq!(StudyplanUrl::default().path(), "/studyplan");
        // Tolerant: any case, a week of an old link ignored, the canonical order written.
        let url = StudyplanUrl::parse("sem=2026w&view=DATES&week=2026-10-14&open=12104&row=148369-aaf38&import=mine");
        assert_eq!(url.path(), "/studyplan?sem=2026W&view=dates&open=12104&row=148369-aaf38&import=mine");
        assert_eq!(url.semester(), SemesterKey::new(2026, true));
        assert_eq!(
            url,
            StudyplanUrl {
                sem: Some("2026W".into()),
                view: PlanView::Dates,
                open: Some("12104".into()),
                row: Some("148369-aaf38".into()),
                import: Some("mine".into()),
                variant: 1,
                share: None,
            }
        );
        let back = |url: &StudyplanUrl| StudyplanUrl::parse(url.path().split_once('?').map(|(_, query)| query).unwrap_or_default());
        assert_eq!(back(&url), url);
        let program = StudyplanUrl::parse("view=all&import=bachelor-informatik-2008&variant=2&sem=2027S");
        assert_eq!(program.path(), "/studyplan?sem=2027S&view=all&import=bachelor-informatik-2008&variant=2");
        assert_eq!(back(&program), program);
        assert_eq!(StudyplanUrl::parse("row=148369-AAF38&open=12104").row.as_deref(), Some("148369-aaf38"));
        for view in PlanView::ALL {
            assert_eq!(PlanView::from_code(view.code()), Some(*view));
            assert_eq!(back(&StudyplanUrl::default().with_view(*view)).view, *view);
        }
        assert_eq!(PlanView::ALL.iter().map(|view| view.label(Locale::De)).collect::<Vec<_>>(), ["Woche", "Termine", "Prüfungen", "Übersicht"]);
    }

    #[test]
    fn hostile_studyplan_values_are_ignored() {
        let url = StudyplanUrl::parse(
            "sem=2026X&view=calendar&open=../../etc&row=148369-zzzzz&import=%3Cscript%3E&variant=2&week=x&modules=12104,12107",
        );
        assert_eq!(url, StudyplanUrl::default());
        for sem in ["1999W", "2100S", "26W", "2026", "2026WS", "+026W"] {
            assert_eq!(StudyplanUrl::parse(&format!("sem={sem}")).sem, None, "{sem}");
        }
        // A Termin without its module, a plan variant without a program: nothing to point at.
        assert_eq!(StudyplanUrl::parse("row=148369-aaf38").row, None);
        assert_eq!(StudyplanUrl::parse("variant=3").path(), "/studyplan");
        assert_eq!(StudyplanUrl::parse("import=mine&variant=0").variant, 1);
        assert_eq!(StudyplanUrl::parse("import=mine&variant=999").variant, 1);
        assert_eq!(StudyplanUrl::parse("import=mine&variant=-2").variant, 1);
        assert_eq!(StudyplanUrl::parse("open=12104&row=0-aaf38").row, None);
        assert_eq!(StudyplanUrl::parse("open=12104&row=148369-aaf3").row, None);
        assert_eq!(StudyplanUrl::parse("open=12104&row=4294967296-aaf38").row, None);
        assert_eq!(StudyplanUrl::parse(&format!("import={}", "a".repeat(121))).import, None);
        assert_eq!(StudyplanUrl::parse(&format!("import={}", "a".repeat(120))).import.map(|slug| slug.len()), Some(120));
        assert_eq!(StudyplanUrl::parse("import=Bachelor-Informatik-2008").import.as_deref(), Some("bachelor-informatik-2008"));
        assert_eq!(StudyplanUrl::parse("import=a+b").import, None);
        assert_eq!(StudyplanUrl::parse("import=a/b").import, None);
    }

    #[test]
    fn studyplan_links_say_what_changes() {
        let url = StudyplanUrl::parse("sem=2026W&view=dates&open=12104&row=148369-aaf38");
        let winter = SemesterKey::new(2026, true);
        let summer = SemesterKey::new(2027, false);
        // Another semester closes the module beside the plan and keeps the view.
        assert_eq!(url.with_semester(summer).path(), "/studyplan?sem=2027S&view=dates");
        assert_eq!(url.with_semester(None).path(), "/studyplan?view=dates");
        assert_eq!(url.with_view(PlanView::Week).path(), "/studyplan?sem=2026W&open=12104&row=148369-aaf38");
        let row = RowKey::parse("148369-a4d12");
        assert_eq!(url.with_open(Some("12107"), None).path(), "/studyplan?sem=2026W&view=dates&open=12107");
        assert_eq!(url.with_open(Some("12104"), row).path(), "/studyplan?sem=2026W&view=dates&open=12104&row=148369-a4d12");
        assert_eq!(url.with_open(None, row).path(), "/studyplan?sem=2026W&view=dates");
        assert_eq!(StudyplanUrl::default().with_semester(winter).path(), "/studyplan?sem=2026W");
        // Taking a Regelstudienplan over ends in the overview, without the program.
        let import = StudyplanUrl::parse("view=all&import=bachelor-informatik-2008&variant=2");
        assert_eq!(import.without_import().with_view(PlanView::Overview).path(), "/studyplan?view=all");
        assert_eq!(import.without_import().variant, 1);
    }

    #[test]
    fn a_shared_plan_stays_in_the_address_until_it_is_answered() {
        let key = SemesterKey::parse("2026W").unwrap();
        let code = SharedPlan::of(key, &["12104".to_string(), "11101".to_string()], Some("079-82-2008")).unwrap().code().unwrap();
        assert_eq!(share_path(&code), format!("/studyplan?share={code}"));
        assert_eq!(StudyplanUrl::parse(&format!("share={code}")).share.as_deref(), Some(code.as_str()));
        let url = StudyplanUrl::parse(&format!("share={code}&view=dates"));
        assert_eq!(url.share.as_deref(), Some(code.as_str()));
        assert_eq!(url.path(), format!("/studyplan?view=dates&share={code}"));
        // Looking around keeps it; taking it over or putting it aside ends it.
        assert_eq!(url.with_view(PlanView::Week).share, url.share);
        assert_eq!(url.with_open(Some("12104"), None).share, url.share);
        assert_eq!(url.without_share().path(), "/studyplan?view=dates");
        // What is no code of a plan is no share.
        for wrong in ["", "x", "12104", &code[1..]] {
            assert_eq!(StudyplanUrl::parse(&format!("share={wrong}")).share, None, "{wrong}");
        }
    }

    #[test]
    fn a_module_page_carries_a_hint_of_where_to_plan() {
        assert_eq!(ModuleHint::parse(""), ModuleHint::default());
        assert_eq!(ModuleHint::default().query(), "");
        let hint = ModuleHint::parse("fill=p3&plan=2026w&utm=x");
        assert_eq!(hint, ModuleHint { plan: Some("2026W".into()), fill: Some(3) });
        assert_eq!(hint.query(), "?plan=2026W&fill=p3");
        assert_eq!(ModuleHint::parse(hint.query().trim_start_matches('?')), hint);
        assert_eq!(format!("{}{}", module_path("12104"), hint.query()), "/catalog/module/12104?plan=2026W&fill=p3");
        assert_eq!(ModuleHint { fill: None, ..hint.clone() }.query(), "?plan=2026W");
        assert_eq!(ModuleHint { plan: None, ..hint }.query(), "?fill=p3");
        assert_eq!(ModuleHint::parse("plan=2026&fill=3"), ModuleHint::default());
        assert_eq!(ModuleHint::parse("plan=%3Cscript%3E&fill=p0"), ModuleHint::default());
    }

    #[test]
    fn program_ids_have_a_shape() {
        for id in ["079-82-2008", "G29-82-2025", "013-D8-2022", "a-b-c", "12345678-1-2-3-4"] {
            assert!(is_program_id(id), "{id}");
        }
        for id in ["", "079-82", "079822008", "079-82-2008-1-2-3", "123456789-82-2008", "079--2008", "-79-82-2008", "079-82-2008-", "079-82-2008 ", "079_82_2008", "07ä-82-2008", "079-82-%20"] {
            assert!(!is_program_id(id), "{id}");
        }
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
        assert_eq!(CatalogUrl::parse("program=x&area=-3").query.program.map(|scope| scope.areas), Some(Vec::new()));
        assert_eq!(CatalogUrl::parse("program=x&area=12").path(), "/catalog?program=x&area=12");
        // Several areas, as a row of the plan means them; each once, in the order given.
        assert_eq!(CatalogUrl::parse("program=x&area=12,7,x,12,-1").query.program.map(|scope| scope.areas), Some(vec![12, 7]));
        assert_eq!(CatalogUrl::parse("program=x&area=12,7").path(), "/catalog?program=x&area=12,7");
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
        // „Vollbild" stays among the marked modules: the module fills the list's place, and
        // beside the list again with `with_full(false)` or `with_open`.
        let full = url.with_full(true);
        assert_eq!(full.path(), "/bookmarks?turnus=winter&sort=ects&desc=1&open=11101&full=1");
        assert_eq!(BookmarksUrl::parse(full.path().split_once('?').map(|(_, query)| query).unwrap_or_default()), full);
        assert_eq!(full.with_full(false), url);
        assert_eq!(full.with_open(Some("12204")).path(), "/bookmarks?turnus=winter&sort=ects&desc=1&open=12204");
        assert_eq!(full.with_open(None).with_full(true).path(), "/bookmarks?turnus=winter&sort=ects&desc=1");
        assert!(!BookmarksUrl::parse("full=1").full && !BookmarksUrl::parse("open=11101&full=yes").full);
        // The order of marking has one direction, and nonsense is ignored.
        assert_eq!(BookmarksUrl::parse("sort=added&desc=1").path(), "/bookmarks");
        assert_eq!(BookmarksUrl::parse("sort=random&turnus=spring&open=../../etc&full=1&desc=1"), BookmarksUrl::default());
        assert!(is_module_id("11101") && is_module_id("FÜS-1".replace('Ü', "U").as_str()));
        assert!(!is_module_id("") && !is_module_id("1 OR 1=1") && !is_module_id(&"9".repeat(33)) && !is_module_id("a\tb"));
    }

    #[test]
    fn local_views_read_and_write_open_and_full_alike() {
        let read = |query: &str| local_from_pairs(&parse_pairs(query));
        assert_eq!(read("open=+11101+&full=1"), (Some("11101".to_string()), true));
        assert_eq!(read("open=11101&open=12204&full=1&full=0"), (Some("11101".to_string()), true), "the first of each counts");
        assert_eq!(read("full=1"), (None, false));
        assert_eq!(read("open=1%20OR%201&full=1"), (None, false));
        assert_eq!(read("open=11101&full=true"), (Some("11101".to_string()), false));
        assert_eq!(local_pairs(Some("11101"), true), vec![("open", "11101".to_string()), ("full", "1".to_string())]);
        assert_eq!(local_pairs(Some("11101"), false), vec![("open", "11101".to_string())]);
        assert!(local_pairs(None, true).is_empty());
        // Both pages that have them write them the same way, at the end of the address.
        let program = ProgramUrl::parse("x", ProgramTab::Plan, "full=1&open=11101&variant=2");
        let marked = BookmarksUrl::parse("full=1&open=11101&sort=title");
        assert_eq!((program.query(), marked.path()), ("?variant=2&open=11101&full=1".to_string(), "/bookmarks?sort=title&open=11101&full=1".to_string()));
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
    fn search_engines_list_pages_not_views() {
        // Every address without a query, the further pages of the unfiltered catalog and the plans
        // of further study directions, as the app writes their links.
        for page in [HOME, CATALOG, PROGRAMS, IMPRINT, "/catalog/module/11101", "/programs/x", "/programs/x/plan", "/programs/x/areas", "/programs#fakultaet-1"] {
            assert!(listed(page), "{page}");
        }
        for n in [2, 3, 24] {
            let (catalog, plan) = (CatalogUrl::default().with_page(n).path(), ProgramUrl::new("x", ProgramTab::Plan).with_variant(n as usize).path());
            assert!(listed(&catalog) && listed(&plan), "{catalog} {plan}");
        }
        // Filters, orders and searches of the lists, the pages of a filtered list, what the app
        // lays beside a page; and what the visitor keeps in their browser.
        let filtered = CatalogUrl::parse("turnus=winter");
        let views = [
            filtered.path(),
            filtered.with_page(2).path(),
            CatalogUrl::parse("sort=title").path(),
            CatalogUrl::parse("q=analysis").path(),
            CatalogUrl::default().with_page(2).with_open(Some("11101")).path(),
            CatalogUrl::default().with_fill(Some(3)).path(),
            program_catalog_path("x", None),
            ProgramsUrl { levels: vec![LevelGroup::Master], ..Default::default() }.path(),
            ProgramUrl::new("x", ProgramTab::Plan).with_variant(2).with_open(Some("11101")).path(),
            ProgramUrl::new("x", ProgramTab::Areas).with_area(Some(3)).path(),
            ProgramUrl::new("x", ProgramTab::Areas).with_variant(2).path(),
            program_path("x", ProgramTab::MyPlan),
            BOOKMARKS.to_string(),
            BookmarksUrl::parse("turnus=winter").path(),
            STUDYPLAN.to_string(),
            StudyplanUrl { import: Some("x".to_string()), ..Default::default() }.path(),
            format!("{}{}", module_path("11101"), ModuleHint::parse("plan=2026W").query()),
        ];
        for view in &views {
            assert!(!listed(view), "{view}");
        }
        // What a hand-written address adds makes no page of its own.
        for address in ["/catalog?page=2&q=x", "/catalog?page=two", "/catalog?", "/?utm_source=x", "/programs/x/plan?utm_source=x", "/programs/x?variant=2"] {
            assert!(!listed(address), "{address}");
        }
    }

    #[test]
    fn paths() {
        assert_eq!(module_path("11101"), "/catalog/module/11101");
        assert_eq!(program_path("bachelor-informatik-2008", ProgramTab::Areas), "/programs/bachelor-informatik-2008/areas");
        assert_eq!(ProgramTab::from_segment("my-plan"), Some(ProgramTab::MyPlan));
        assert_eq!(ProgramTab::from_segment("electives"), None);
        assert_eq!(program_catalog_path("bachelor-informatik-2008", None), "/catalog?program=bachelor-informatik-2008");
        assert_eq!(program_catalog_path("x", Some("11101")), "/catalog?program=x&open=11101");
        assert!(ProgramTab::Plan.indexed() && !ProgramTab::MyPlan.indexed());
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
        // An area opened out of the row keeps the row, and so does a module opened out of that.
        let within = plan.with_area_keeping_req(3);
        assert_eq!(within.query(), "?variant=2&area=3&req=7");
        assert_eq!(within.with_open(Some("11101")).query(), "?variant=2&area=3&req=7&open=11101");
        assert_eq!(within.with_req(Some(7)).query(), "?variant=2&req=7");
        assert_eq!(areas.with_area_keeping_req(4).query(), "?area=4");
        // The module fills the page: only with a module, and beside the page again with `with_open`.
        let full = ProgramUrl::parse("x", ProgramTab::Areas, "area=12&open=11101&full=1");
        assert!(full.full);
        assert_eq!(full.query(), "?area=12&open=11101&full=1");
        assert_eq!(full.with_open(Some("11101")).query(), "?area=12&open=11101");
        assert_eq!(full.with_open(None).query(), "?area=12");
        assert_eq!(areas.with_full(true).query(), "?area=12&open=11101&full=1");
        assert!(!ProgramUrl::parse("x", ProgramTab::Plan, "full=1").full);
        assert!(!ProgramUrl::parse("x", ProgramTab::Plan, "open=11101&full=yes").full);
        assert_eq!(full.with_full(false).query(), "?area=12&open=11101");
        assert_eq!(LocalView::path(&full), full.path());
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
