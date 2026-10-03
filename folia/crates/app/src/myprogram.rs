//! „Mein Studiengang" in this browser: the visitor's program, study direction, Studienbeginn and
//! town, and what the local catalog knows of that program.
//!
//! Owner clarification (2026-09-24): „Mein Studiengang" is the app's alone and kept in
//! `localStorage` (`betula.myprogram.v1`, the text of `folia_plans::studyplan::MineDoc`), like the
//! Merkliste: empty on the server (R9), in no request, and never dragged along in addresses. The
//! store is where every page reads it (the Studienplan's Fachsemester and import, the finder,
//! the Standort); the program's slug stands only in a catalog address that is filtered by it.
//!
//! Where the app offers it (A.10): set on the program's page (`MineButton`); the catalog's tab
//! leads to the program's catalog on its first entry of a session (`MineResolved::catalog_href`,
//! `tabs::Tabs::href_with_root`); the catalog's program picker lists it first; the program
//! overview names it; links to its page show the stored Studienrichtung (`program_href`). All of
//! it only while the stored program is in the snapshot (`MyProgramInfo::exact`).

use folia_calendar::select::TownChoice;
use folia_calendar::semester::SemesterKey;
use folia_model::rows::Program;
use folia_pages::ask::{MyProgramAsk, PlanSourceAsk};
use folia_pages::MyProgramInfo;
use folia_plans::studyplan::MineDoc;
use folia_plans::variants::{PlanVariant, Supplement};
use folia_routes::filter::{CatalogQuery, ProgramScope};
use folia_routes::url::{self, CatalogUrl, ProgramTab, ProgramUrl};
use leptos::prelude::*;

use folia_data::{use_data, DataClient};
use crate::i18n;
use folia_design::nav;
use folia_design::ui::Icon;

const STORAGE_KEY: &str = "betula.myprogram.v1";

/// The browser app (`csr`): only there is anything stored.
const APP: bool = cfg!(feature = "csr");

/// „Mein Studiengang", shared through context. Empty on the server, always.
#[derive(Clone, Copy)]
pub struct MyProgram(RwSignal<MineDoc>);

impl MyProgram {
    /// Reads what this browser has stored and provides it; another tab of the same browser may
    /// change it, and this one follows.
    pub fn provide() -> Self {
        let mine = MyProgram(RwSignal::new(load()));
        provide_context(mine);
        // Effects run in the browser only.
        Effect::new(move |_| {
            let handle = window_event_listener_untyped("storage", move |_| {
                let stored = load();
                if mine.0.with_untracked(|doc| *doc != stored) {
                    mine.0.set(stored);
                }
            });
            on_cleanup(move || handle.remove());
        });
        mine
    }

    pub fn expect() -> Option<Self> {
        use_context::<MyProgram>()
    }

    /// All of it. Tracked: read it in a memo that keeps only what it needs.
    pub fn get(self) -> MineDoc {
        self.0.get()
    }

    /// Reads it without copying. Tracked.
    pub fn with<R>(self, f: impl FnOnce(&MineDoc) -> R) -> R {
        self.0.with(f)
    }

    /// The Standort the visitor chose; `Derive` when none. Tracked.
    pub fn town(self) -> TownChoice {
        self.0.with(|doc| doc.town)
    }

    /// The Studienbeginn, when one is stored. Tracked.
    pub fn start(self) -> Option<SemesterKey> {
        self.0.with(|doc| doc.start)
    }

    /// The program (its id, never the slug), its display name as it reads now (said once the id
    /// is gone from the snapshot), the caption of the chosen plan (`""`: the only or the unnamed
    /// one) and the page that fills a core plan's direction row. An id that cannot be one is not
    /// taken; Studienbeginn and Standort stay.
    pub fn set_program(self, id: &str, name: &str, caption: &str, direction: Option<&str>) {
        if !url::is_program_id(id) {
            return;
        }
        self.change(|doc| {
            doc.program = Some(id.to_string());
            doc.name = Some(name.to_string()).filter(|name| !name.trim().is_empty());
            doc.caption = Some(caption.to_string());
            doc.direction = direction.map(str::to_string);
        });
    }

    /// No program any more: its name, plan and direction go with it; Studienbeginn and Standort
    /// stay, since they are the student's whatever the program.
    pub fn clear_program(self) {
        self.change(MineDoc::clear_program);
    }

    pub fn set_start(self, start: Option<SemesterKey>) {
        self.change(|doc| doc.start = start);
    }

    pub fn set_town(self, town: TownChoice) {
        self.change(|doc| doc.town = town);
    }

    /// Changes and stores it, telling what depends on it only when something changed. Nothing left
    /// takes the key out (`nav::local_set`).
    fn change(self, f: impl FnOnce(&mut MineDoc)) {
        let mut doc = self.0.try_with_untracked(Clone::clone).unwrap_or_default();
        f(&mut doc);
        if self.0.try_with_untracked(|now| *now != doc).unwrap_or(false) {
            nav::local_set(STORAGE_KEY, &doc.stored());
            self.0.set(doc);
        }
    }
}

/// What this browser has stored; nothing on the server and in a browser that refuses storage.
fn load() -> MineDoc {
    restored(nav::local_get(STORAGE_KEY).as_deref())
}

/// „Mein Studiengang" of a stored text, read like anything from outside: never an error.
fn restored(stored: Option<&str>) -> MineDoc {
    stored.map(MineDoc::restored).unwrap_or_default()
}

/// The stored program as the local catalog has it (`pages::my_program`): the program itself, or,
/// when its PO is gone from the snapshot, the newest of its family (`exact` false: then no page
/// sets a default from it, A.10). `None` without a program, and on the server, which never knows
/// one (R9). One answer for the whole app, from the visit's cached `programs`.
#[derive(Clone, Copy)]
pub struct MineResolved(pub Memo<Option<MyProgramInfo>>);

impl MineResolved {
    /// Call it after `MyProgram::provide`.
    pub fn provide() -> Self {
        let mine = MyProgram::expect();
        let source = use_data().ok();
        // The program alone: a changed Studienbeginn or Standort asks the catalog nothing.
        let program = Memo::new(move |_| if APP { mine.and_then(|mine| mine.with(|doc| doc.program.clone())) } else { None });
        let resolved = MineResolved(Memo::new(move |before| {
            let id = program.get()?;
            folia_pages::ask::unless_pending(source.as_ref()?.now(&MyProgramAsk { program_id: id.clone() }), before, |now| now.ok().flatten())
        }));
        provide_context(resolved);
        resolved
    }

    pub fn expect() -> Option<Self> {
        use_context::<MineResolved>()
    }

    /// The stored program, while it is in the snapshot: only then does anything default to it
    /// (A.10). Tracked.
    pub fn exact(self) -> Option<Program> {
        self.0.with(|info| info.as_ref().filter(|info| info.exact).map(|info| info.program.clone()))
    }

    /// The catalog as a way into it: filtered by „Mein Studiengang" while the stored program is in
    /// the snapshot, else the whole catalog. The app's path, without the language's prefix: what
    /// writes it into a page writes `Texts::path` of it. Tracked.
    pub fn catalog_href(self) -> String {
        self.0.with(|info| catalog_href(info.as_ref()))
    }
}

/// „Informatik B.Sc. · PO 2008": a program as „Mein Studiengang" stores its name and the app names
/// it wherever it means the visitor's.
pub fn program_name(program: &Program) -> String {
    format!("{} {} · PO {}", program.name, program.degree(), po_of(program))
}

/// „2008": the year of a program's regulation, else its version.
pub fn po_of(program: &Program) -> String {
    program.po_year.map(|year| year.to_string()).unwrap_or_else(|| program.po_version.clone())
}

/// The catalog filtered by the program (`program=<slug>`), the one kind of address „Mein
/// Studiengang" may stand in (A.10), where the stored program is in the snapshot; else the whole
/// catalog. The app's path, without the language's prefix (it is compared and remembered, too).
pub fn catalog_href(info: Option<&MyProgramInfo>) -> String {
    match info.filter(|info| info.exact) {
        Some(info) => {
            let program = ProgramScope { program_slug: info.program.slug.clone(), ..Default::default() };
            CatalogUrl { query: CatalogQuery { program: Some(program), ..Default::default() }, ..Default::default() }.path()
        }
        None => url::CATALOG.to_string(),
    }
}

/// The program's page as an app-made link to the visitor's own program shows it: with the plan of
/// the stored Studienrichtung (`variant=`, A.10) — the page stored as the direction, else the plan
/// whose caption was stored, the first where none or none of that caption is. The app's path,
/// without the language's prefix: what writes it into a page writes `Texts::path` of it.
pub fn program_href(source: Option<&DataClient>, program: &Program, caption: Option<&str>, direction: Option<&str>) -> String {
    let place = caption.filter(|caption| !caption.trim().is_empty()).and_then(|caption| {
        let plans = source?.now(&PlanSourceAsk { program_id: program.id.clone(), locale: crate::i18n::locale() }).ok()??;
        ProgramPlans::new(&plans.variants, plans.supplements).place(caption, direction)
    });
    ProgramUrl::new(&program.slug, ProgramTab::Plan).with_variant(place.map_or(1, |place| place.shown() + 1)).path()
}

/// Where a plan stands among a program's plans: a core plan (its index in `plan_variants`' result),
/// and the page that fills a row of it, where it is one of those.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place {
    core: usize,
    page: Option<usize>,
}

impl Place {
    /// The plan the program's page shows for it: the page, else the core plan.
    fn shown(self) -> usize {
        self.page.unwrap_or(self.core)
    }
}

/// A program's plans as „Mein Studiengang" keeps and names them (A.10). The store keeps a plan by
/// its caption; a page that fills a row of a core plan („Studienplan · Seite 18") it keeps as the
/// core's caption with the page as the direction (the import's words, C.16). The program's page
/// names each by its chip label.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgramPlans {
    /// Each plan's caption (`PlanVariant::full`) and its label, in the order of the page.
    plans: Vec<(String, String)>,
    /// The pages among them (`variants::supplements`).
    pages: Vec<Supplement>,
}

impl ProgramPlans {
    pub fn new(plans: &[PlanVariant], pages: Vec<Supplement>) -> Self {
        Self { plans: plans.iter().map(|plan| (plan.full.clone(), plan.label.clone())).collect(), pages }
    }

    /// Where the plan at `index` (0-based; past the last, the last, as the page shows it) stands.
    fn place_of(&self, index: usize) -> Place {
        let index = index.min(self.plans.len().saturating_sub(1));
        match self.pages.iter().find(|supplement| supplement.page == index) {
            Some(supplement) => Place { core: supplement.core, page: Some(index) },
            None => Place { core: index, page: None },
        }
    }

    /// The plan at `index` (0-based) as the store keeps it: its caption (`""` for the unnamed one,
    /// and where there is none), and for a page its core's caption with the page as the direction.
    fn kept(&self, index: usize) -> (String, Option<String>) {
        let place = self.place_of(index);
        let caption = |at: usize| self.plans.get(at).map(|(full, _)| full.clone());
        (caption(place.core).unwrap_or_default(), place.page.and_then(caption))
    }

    /// Where a stored caption and direction stand among these plans: the only plan whatever was
    /// stored; `None` where the caption names none of them (a program picked in the Studienplan,
    /// which keeps no Studienrichtung). A direction that names no page of the core is left out.
    fn place(&self, caption: &str, direction: Option<&str>) -> Option<Place> {
        if self.plans.len() == 1 {
            return Some(Place { core: 0, page: None });
        }
        let at = self.plans.iter().position(|(full, _)| full.trim() == caption.trim())?;
        let place = self.place_of(at);
        if place.page.is_some() {
            return Some(place);
        }
        let named = |page: &usize| self.plans.get(*page).is_some_and(|(full, _)| direction.is_some_and(|direction| full.trim() == direction.trim()));
        let page = self.pages.iter().filter(|supplement| supplement.core == place.core).map(|supplement| supplement.page).find(named);
        Some(Place { page, ..place })
    }

    /// „PA und IoT", „Seite 18": what the program's page calls the plan of a place.
    fn label(&self, place: Place) -> Option<String> {
        self.plans.get(place.shown()).map(|(_, label)| label.clone()).filter(|label| !label.trim().is_empty())
    }

    /// Whether a click on the plan `shown` (`None`: no plan shown) would keep nothing new, where
    /// the store holds this program at `kept` (`None`: it holds another program or none;
    /// `Some(None)`: this one, without a Studienrichtung among these plans).
    fn holds(&self, kept: Option<Option<Place>>, shown: Option<usize>) -> bool {
        match (kept, shown) {
            (None, _) => false,
            (Some(Some(place)), Some(index)) => self.place_of(index) == place,
            (Some(_), _) => true,
        }
    }
}

/// „Als meinen Studiengang setzen" among the actions of a program's page (A.10): stores the program
/// with the Studienrichtung shown (`ProgramPlans::kept`: for a page that fills a core plan's
/// direction row, its core's caption with the page as the direction; on a tab without the plan,
/// none); pressed, „Mein Studiengang", a click takes it away again (Studienbeginn and Standort
/// stay).
///
/// Pressed means that a click would store nothing new: this program, and on the plan's tab the plan
/// shown. On another plan of the same program the button is not pressed, and its title names the
/// kept one a click replaces („Ersetzt: PA und IoT"), as it names another program. A program kept
/// without a Studienrichtung, or with one none of its plans has any more, is pressed on every plan.
///
/// Part of server HTML like `MarkButton`: unpressed, the same for everybody (R9), kept in its place
/// but not shown until the app runs (`.mine-toggle`), so the actions do not move at the takeover
/// (R15). A click flips the button at once and stores after the next frame (R21): what follows
/// from the program (the catalog's tab, what the local catalog says of it) comes after the button
/// has answered.
#[component]
pub fn MineButton(
    #[prop(into)] program_id: String,
    /// The program as „Mein Studiengang" names it (`program_name`).
    #[prop(into)]
    name: String,
    plans: ProgramPlans,
    /// The plan shown (0-based); `None` on a tab without the plan, and for a program without one.
    #[prop(into)]
    shown: Signal<Option<usize>>,
) -> impl IntoView {
    let t = i18n::t();
    let mine = MyProgram::expect().filter(|_| APP);
    let plans = StoredValue::new(plans);
    // One memo each (R5), both from the store alone: whether this program is kept and where its
    // Studienrichtung stands among the plans (`Some(None)`: nowhere), and another program kept
    // instead.
    let kept = {
        let id = program_id.clone();
        Memo::new(move |_| {
            let mine = mine?;
            mine.with(|doc| {
                let this = doc.program.as_deref() == Some(id.as_str());
                this.then(|| plans.with_value(|plans| plans.place(doc.caption.as_deref().unwrap_or_default(), doc.direction.as_deref())))
            })
        })
    };
    let replaced = {
        let id = program_id.clone();
        Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| replaced_name(doc, &id))))
    };
    let stored = Memo::new(move |_| {
        let (kept, shown) = (kept.get(), shown.get());
        plans.with_value(|plans| plans.holds(kept, shown))
    });
    // What the last click said, until the store has it.
    let said = RwSignal::new(None::<bool>);
    let pressed = Memo::new(move |_| said.get().unwrap_or_else(|| stored.get()));
    let toggle = move |_: leptos::ev::MouseEvent| {
        let Some(mine) = mine else { return };
        let was = pressed.get_untracked();
        said.set(Some(!was));
        // Unpressed on a tab without the plan, the program is not kept yet, and nothing says which
        // of its plans is meant: no Studienrichtung, as the Studienplan's picker keeps it.
        let (caption, direction) = shown.get_untracked().map(|index| plans.with_value(|plans| plans.kept(index))).unwrap_or_default();
        let (id, name) = (program_id.clone(), name.clone());
        nav::after_paint(move || {
            if was {
                mine.clear_program();
            } else {
                mine.set_program(&id, &name, &caption, direction.as_deref());
            }
            said.try_set(None);
        });
    };
    let label = move || if pressed.get() { t.myprogram.mine } else { t.myprogram.set_mine };
    let tip = move || match (pressed.get(), replaced.get(), kept.get()) {
        (true, ..) => Some(t.myprogram.unset_title.to_string()),
        (false, Some(other), _) => Some((t.myprogram.replaces)(&other)),
        (false, None, Some(Some(place))) => plans.with_value(|plans| plans.label(place)).map(|label| (t.myprogram.replaces)(&label)),
        (false, None, _) => None,
    };
    view! {
        <button
            class="action mine-toggle"
            type="button"
            on:click=toggle
            aria-pressed=move || if pressed.get() { "true" } else { "false" }
            aria-busy=move || said.get().map(|_| "true")
            title=tip
        >
            <Icon name="star"/><span>{label}</span>
        </button>
    }
}

/// The name of another program stored as „Mein Studiengang" than `program_id`, which setting this
/// one replaces: its stored name, else its id.
fn replaced_name(doc: &MineDoc, program_id: &str) -> Option<String> {
    let other = doc.program.as_deref().filter(|stored| *stored != program_id)?;
    Some(doc.name.clone().filter(|name| !name.trim().is_empty()).unwrap_or_else(|| other.to_string()))
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Town;

    use super::*;

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        assert_eq!(restored(None), MineDoc::default());
        // Garbage: nothing of it is taken for a program, a start or a town.
        let garbage = restored(Some("program\t../../etc\nstart\t2026X\ntown\tberlin\n\u{0}\n"));
        assert_eq!((garbage.program, garbage.start, garbage.town), (None, None, TownChoice::Derive));
        let good = restored(Some("program\t079-82-2008\nstart\t2026W\ntown\tsenftenberg\n"));
        assert_eq!((good.program.as_deref(), good.start, good.town), (Some("079-82-2008"), SemesterKey::parse("2026W"), TownChoice::Only(Town::Senftenberg)));
    }

    #[test]
    fn nothing_left_takes_the_key_out() {
        // What `nav::local_set` is handed: empty means the key goes.
        assert_eq!(MineDoc::default().stored(), "");
        let mut doc = restored(Some("program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n"));
        doc.clear_program();
        // The Studienbeginn is the student's whatever the program: it stays, and so does the key.
        assert_eq!(doc.stored(), "start\t2026W\n");
        doc.start = None;
        assert_eq!(doc.stored(), "");
    }

    fn informatik(po_year: Option<i64>) -> Program {
        Program {
            id: "079-82-2008".into(),
            slug: "bachelor-informatik-2008".into(),
            name: "Informatik".into(),
            degree_level: folia_model::labels::Code::parse("bachelor"),
            study_variant: None,
            degree_label: Some("B.Sc.".into()),
            degree_raw: "Bachelor".into(),
            degree_display: Some("B.Sc.".into()),
            po_version: "2008 - 2. SÄ 2024".into(),
            po_year,
            family_key: "079-82".into(),
            name_key: "informatik".into(),
            is_latest_po: true,
            source_url: String::new(),
            has_plan: true,
            plan_status: None,
            curricular_modules: 0,
            fues_modules: 0,
            documents: 0,
        }
    }

    #[test]
    fn mein_studiengang_is_named_and_filtered_by_only_while_it_is_in_the_snapshot() {
        assert_eq!(program_name(&informatik(Some(2008))), "Informatik B.Sc. · PO 2008");
        assert_eq!(program_name(&informatik(None)), "Informatik B.Sc. · PO 2008 - 2. SÄ 2024");
        // The catalog of the program: its slug, the one address it may stand in, and only while
        // the stored PO is in the snapshot.
        let exact = MyProgramInfo { program: informatik(Some(2008)), exact: true, latest: None };
        assert_eq!(catalog_href(Some(&exact)), "/catalog?program=bachelor-informatik-2008");
        let gone = MyProgramInfo { exact: false, ..exact.clone() };
        assert_eq!((catalog_href(Some(&gone)), catalog_href(None)), ("/catalog".to_string(), "/catalog".to_string()));
        // Without a caption, or without a source to look the plans up in, the first plan.
        assert_eq!(program_href(None, &exact.program, Some("Studienrichtung A"), None), "/programs/bachelor-informatik-2008/plan");
    }

    /// Plans as the program's page lists them: a core plan whose row „Seite 18" fills, and
    /// another one.
    fn three_plans() -> ProgramPlans {
        let plan = |full: &str, label: &str| (full.to_string(), label.to_string());
        ProgramPlans {
            plans: vec![plan("Kernplan", "Kern"), plan("Studienplan · Seite 18", "Seite 18"), plan("Andere Richtung", "Andere")],
            pages: vec![Supplement { core: 0, ord: 16, page: 1 }],
        }
    }

    /// „Mein Studiengang" keeps the plan shown by its caption; a page that fills a core plan's row
    /// is kept as the core with the page as its direction, the way the import stores it.
    #[test]
    fn mein_studiengang_keeps_the_plan_shown() {
        let plans = three_plans();
        assert_eq!(plans.kept(0), ("Kernplan".to_string(), None));
        assert_eq!(plans.kept(1), ("Kernplan".to_string(), Some("Studienplan · Seite 18".to_string())));
        assert_eq!(plans.kept(2), ("Andere Richtung".to_string(), None));
        // An address naming a plan past the last shows the last, as the page does; no plan at all
        // is the unnamed one.
        assert_eq!(plans.kept(8), ("Andere Richtung".to_string(), None));
        assert_eq!(ProgramPlans::default().kept(0), (String::new(), None));
    }

    /// What is kept is found again among the plans: a page by its core and itself as the
    /// direction, the only plan whatever was kept, nothing for a caption none of them has.
    #[test]
    fn a_kept_studienrichtung_is_found_among_the_plans() {
        let plans = three_plans();
        let core = Place { core: 0, page: None };
        let page = Place { core: 0, page: Some(1) };
        assert_eq!(plans.place("Kernplan", None), Some(core));
        assert_eq!(plans.place(" Kernplan ", Some("Studienplan · Seite 18")), Some(page));
        // A direction that is no page of that core does not count; a page kept as the caption is
        // still that page.
        assert_eq!(plans.place("Kernplan", Some("Andere Richtung")), Some(core));
        assert_eq!(plans.place("Studienplan · Seite 18", None), Some(page));
        assert_eq!(plans.place("Andere Richtung", Some("Studienplan · Seite 18")), Some(Place { core: 2, page: None }));
        // Picked in the Studienplan (no caption), or a caption of an older snapshot.
        assert_eq!(plans.place("", None), None);
        assert_eq!(plans.place("Vertiefung B", None), None);
        let only = ProgramPlans { plans: vec![("Regelstudienplan".to_string(), "Regelstudienplan".to_string())], pages: Vec::new() };
        assert_eq!(only.place("", None), Some(core));
        assert_eq!((plans.label(page).as_deref(), plans.label(core).as_deref()), (Some("Seite 18"), Some("Kern")));
    }

    /// Pressed means a click would keep nothing new: this program and, where a plan is shown, that
    /// plan. Another plan of the same program is not pressed, so a click keeps it instead of
    /// taking the program away.
    #[test]
    fn the_button_is_pressed_on_the_plan_that_is_kept() {
        let plans = three_plans();
        let page = Some(Some(Place { core: 0, page: Some(1) }));
        assert!(plans.holds(page, Some(1)));
        assert!(!plans.holds(page, Some(0)), "the core plan without the page is another Studienrichtung");
        assert!(!plans.holds(page, Some(2)));
        // On a tab without the plan the program is what counts.
        assert!(plans.holds(page, None));
        // Kept without a Studienrichtung of these plans: every plan is the program's.
        assert!(plans.holds(Some(None), Some(2)) && plans.holds(Some(None), None));
        // Another program, or none.
        assert!(!plans.holds(None, Some(1)) && !plans.holds(None, None));
    }

    /// The snapshot the catalog's tests read: `FOLIA_TEST_SNAPSHOT`, else the one
    /// `snapshot/current.json` names.
    fn snapshot() -> DataClient {
        use std::path::PathBuf;
        use std::sync::Mutex;

        use folia_model::native::NativeDatabase;
        use folia_model::{Database, DbError};

        use folia_data::CatalogSource;

        struct Snapshot(Mutex<NativeDatabase>);
        impl CatalogSource for Snapshot {
            fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError> {
                let db = self.0.lock().map_err(|_| DbError::Unavailable("the test snapshot is poisoned".to_string()))?;
                job(&*db);
                Ok(())
            }
        }
        let path = std::env::var("FOLIA_TEST_SNAPSHOT").map(PathBuf::from).unwrap_or_else(|_| {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../snapshot");
            let pointer = std::fs::read_to_string(dir.join("current.json")).expect("set FOLIA_TEST_SNAPSHOT to a catalog-*.db");
            let file = pointer.split("\"file\"").nth(1).and_then(|rest| rest.split('"').nth(1)).expect("snapshot/current.json names a file");
            dir.join(file)
        });
        DataClient::new(folia_data::Source(std::sync::Arc::new(Snapshot(Mutex::new(NativeDatabase::open(&path).expect("the test snapshot opens"))))))
    }

    /// A link to the visitor's own program shows the kept Studienrichtung (A.10), looked up in the
    /// snapshot: Elektrotechnik's second plan, and a „Seite N" page of 370-82-2023 kept as its
    /// core with the page as the direction.
    #[test]
    fn a_link_to_mein_studiengang_shows_the_kept_plan() {
        let source = snapshot();
        let program = |id: &str| source.now(&MyProgramAsk { program_id: id.to_string() }).unwrap().expect("the program is in the snapshot").program;
        let plans = |id: &str| source.now(&PlanSourceAsk { program_id: id.to_string(), locale: folia_locale::Locale::De }).unwrap().expect("the program has plans");

        let elektrotechnik = program("048-82-2022");
        let two = plans("048-82-2022");
        assert!(two.variants.len() >= 2, "Elektrotechnik B.Sc. 2022 has a plan per Studienrichtung");
        let second = two.variants[1].full.clone();
        assert_eq!(program_href(Some(&source), &elektrotechnik, Some(&second), None), "/programs/bachelor-elektrotechnik-2022/plan?variant=2");
        let first = two.variants[0].full.clone();
        assert_eq!(program_href(Some(&source), &elektrotechnik, Some(&first), None), "/programs/bachelor-elektrotechnik-2022/plan");
        // A caption none of its plans has, or none at all: the first plan.
        assert_eq!(program_href(Some(&source), &elektrotechnik, Some("Vertiefung B"), None), "/programs/bachelor-elektrotechnik-2022/plan");
        assert_eq!(program_href(Some(&source), &elektrotechnik, Some(""), None), "/programs/bachelor-elektrotechnik-2022/plan");

        let wiing = program("370-82-2023");
        let paged = plans("370-82-2023");
        let supplement = paged.supplements.first().expect("370-82-2023 has a page that fills a row of its core plan").clone();
        let (core, page) = (paged.variants[supplement.core].full.clone(), paged.variants[supplement.page].full.clone());
        let at = |n: usize| match n {
            1 => format!("/programs/{}/plan", wiing.slug),
            n => format!("/programs/{}/plan?variant={n}", wiing.slug),
        };
        assert_eq!(program_href(Some(&source), &wiing, Some(&core), None), at(supplement.core + 1));
        assert_eq!(program_href(Some(&source), &wiing, Some(&core), Some(&page)), at(supplement.page + 1));
        assert_eq!(program_href(Some(&source), &wiing, Some(&core), Some("Studienplan · Seite 99")), at(supplement.core + 1));
    }

    #[test]
    fn setting_a_program_says_which_one_it_replaces() {
        let doc = restored(Some("program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\n"));
        assert_eq!(replaced_name(&doc, "048-82-2022").as_deref(), Some("Informatik B.Sc. · PO 2008"));
        assert_eq!(replaced_name(&doc, "079-82-2008"), None);
        assert_eq!(replaced_name(&MineDoc::default(), "079-82-2008"), None);
        // No name stored: the id says which.
        assert_eq!(replaced_name(&restored(Some("program\t079-82-2008\n")), "048-82-2022").as_deref(), Some("079-82-2008"));
    }
}
