//! Real module trees for the tests of the area pickers (`pages::catalog_areas`) and of the rows
//! of a plan (`plan::areas_for_row`), copied from the snapshot of 2026-09-21 with the kinds the
//! program's sources settle on for their modules (`v_program_module.kind`). `U` is a module no
//! source gives a kind. `examples/area_survey.rs` shows the same for every program.

use crate::labels::{Code, ModuleKind};
use crate::pages::{self, CatalogArea};
use crate::rows_detail::{AreaNode, AreaPlacement, PlanEntry};

const C: Option<ModuleKind> = Some(ModuleKind::Compulsory);
const E: Option<ModuleKind> = Some(ModuleKind::Elective);
const T: Option<ModuleKind> = Some(ModuleKind::Thesis);
const U: Option<ModuleKind> = None;

/// One node: its depth (1 = directly below the PO), its label, and the modules placed directly
/// in it, as (how many, of which kind).
pub type Node = (i64, &'static str, &'static [(usize, Option<ModuleKind>)]);

/// The areas of a tree as the pickers get them.
pub fn areas(nodes: &[Node]) -> Vec<CatalogArea> {
    let (placements, tree) = build(nodes);
    pages::catalog_areas(&placements, &tree)
}

pub fn build(nodes: &[Node]) -> (Vec<AreaPlacement>, Vec<AreaNode>) {
    let mut tree: Vec<AreaNode> = Vec::new();
    let mut placements: Vec<AreaPlacement> = Vec::new();
    let mut above: Vec<(i64, String)> = Vec::new(); // (id, label) of the open nodes, top down
    for (index, (depth, label, modules)) in nodes.iter().enumerate() {
        let id = index as i64 + 1;
        above.truncate((*depth - 1).max(0) as usize);
        let parent = above.last().map(|(id, _)| *id);
        let path = above.iter().map(|(_, label)| label.as_str()).chain([*label]).collect::<Vec<_>>().join(" / ");
        tree.push(AreaNode { id, parent_id: parent, depth: *depth, label: label.to_string(), stated_kind: None });
        let mut n = 0;
        for (count, kind) in modules.iter() {
            for _ in 0..*count {
                n += 1;
                placements.push(AreaPlacement {
                    module_id: format!("{id}{n:03}"),
                    module_title: String::new(),
                    module_credits: None,
                    area_id: id,
                    area: path.clone(),
                    area_label: label.to_string(),
                    depth: *depth,
                    area_ord: id,
                    kind: None,
                    kind_basis: None,
                    module_kind: kind.map(Code::Known),
                });
            }
        }
        above.push((id, label.to_string()));
    }
    (placements, tree)
}

/// Informatik B.Sc., PO 2008 (`bachelor-informatik-2008`).
pub const INFORMATIK_BSC: &[Node] = &[
    (1, "Grundstudium", &[]),
    (2, "Komplex Informatik", &[(9, C)]),
    (3, "Proseminar oder Praktikum", &[(1, U), (2, E)]),
    (2, "Komplex Mathematik", &[(3, C)]),
    (2, "Komplex Nebenfach", &[]),
    (3, "Wahlpflichtmodule Praktische Mathematik", &[(17, E)]),
    (3, "Mathematik", &[(1, U), (22, E)]),
    (3, "Physik", &[(4, U)]),
    (3, "Maschinenbau / Elektrotechnik", &[(16, U)]),
    (3, "Wirtschaftswissenschaften", &[(9, U)]),
    (3, "Bauingenieurwesen", &[(7, U)]),
    (1, "Fachstudium", &[(1, T)]),
    (2, "Grundlagen der Informatik", &[(7, E)]),
    (2, "Praktische Informatik", &[(10, E)]),
    (2, "Angewandte und Technische Informatik", &[(9, E)]),
    (2, "Seminar oder Praktikum aus der Informatik", &[(6, E)]),
];

/// Informatik M.Sc., PO 2008 (`master-informatik-2008`).
pub const INFORMATIK_MSC: &[Node] = &[
    (1, "Informatik-Vertiefung", &[]),
    (2, "Grundlagen der Informatik", &[(6, E)]),
    (2, "Praktische Informatik", &[(2, U), (9, E)]),
    (2, "Angewandte und Technische Informatik", &[(2, U), (24, E)]),
    (2, "Seminare oder Praktika", &[(28, E)]),
    (1, "Komplex Nebenfach", &[]),
    (2, "Mathematik", &[(3, U), (20, E)]),
    (2, "Anwendungen", &[]),
    (3, "Mathematik", &[(5, U), (22, E)]),
    (3, "Physik", &[(4, U)]),
    (3, "Maschinenbau / Elektrotechnik", &[(17, U), (3, E)]),
    (3, "Wirtschaftsingenieurwesen", &[(10, U)]),
    (3, "Bauingenieurwesen", &[(7, U)]),
];

/// Elektrotechnik B.Sc., PO 2022 (`bachelor-elektrotechnik-2022`): four study directions, one
/// plan for MIT and EET, one for PA(u) and IoT.
pub const ELEKTROTECHNIK_BSC: &[Node] = &[
    (1, "Grundstudium", &[]),
    (2, "Elektrotechnik (MIT)", &[(11, C)]),
    (2, "Elektrotechnik (EET)", &[(11, C)]),
    (2, "Elektrotechnik (PAu)", &[(14, C)]),
    (2, "Elektrotechnik (IoT)", &[(14, C)]),
    (2, "Mathematik und Physik (EET)", &[(7, C)]),
    (2, "Mathematik und Physik (MIT)", &[(7, C)]),
    (2, "Mathematik und Physik (PAu)", &[(4, C)]),
    (2, "Mathematik und Physik (IoT)", &[(4, C)]),
    (2, "Informatik (MIT)", &[(1, C)]),
    (3, "Wahlpflichtmodul (MIT)", &[(2, E)]),
    (2, "Informatik (EET)", &[(1, C)]),
    (3, "Wahlpflichtmodul (EET)", &[(2, E)]),
    (2, "Informatik (PAu)", &[(2, C)]),
    (2, "Informatik (IoT)", &[(2, C)]),
    (1, "Hauptstudium", &[(2, U), (1, T)]),
    (2, "Studienrichtungsspezifische Vertiefungsmodule (EET)", &[(23, U)]),
    (2, "Studienrichtungsspezifische Vertiefungsmodule (MIT)", &[(23, U)]),
    (2, "Studienrichtungsspezifische Vertiefungsmodule (PAu)", &[(12, U)]),
    (2, "Studienrichtungsspezifische Vertiefungsmodule (IoT)", &[(15, U)]),
];

/// Elektrotechnik M.Sc., PO 2018 (`master-elektrotechnik-2018-90`): a label that is nothing
/// but a kind, with an area below it.
pub const ELEKTROTECHNIK_MSC: &[Node] = &[
    (1, "Pflichtmodule", &[(4, C)]),
    (1, "Studienrichtung Kommunikationstechnik (KT)", &[]),
    (2, "Pflichtmodule (KT)", &[(4, C)]),
    (2, "Wahlpflichtmodule (KT)", &[(4, C), (11, E)]),
    (3, "Zweite Fremdsprache", &[(2, E)]),
    (1, "Studienrichtung Prozessautomatisierung (PAu)", &[]),
    (2, "Pflichtmodule (PAu)", &[(4, C)]),
    (2, "Wahlpflichtmodule (PAu)", &[(3, C), (11, E)]),
    (3, "Zweite Fremdsprache", &[(2, E)]),
    (1, "Studienrichtung Energiesysteme (ES)", &[]),
    (2, "Pflichtmodule (ES)", &[(2, C)]),
    (2, "Wahlpflichtmodule (ES)", &[(7, C), (9, E)]),
    (3, "Zweite Fremdsprache", &[(2, E)]),
];

/// Architektur B.Sc., PO 2022 (`bachelor-architektur-2022`): an account at the top, the
/// electives of each field in a node called „Wahlpflichtmodule".
pub const ARCHITEKTUR_BSC: &[Node] = &[
    (1, "Gesamtkonto Bachelor", &[]),
    (2, "Entwerfen", &[(1, T)]),
    (3, "Pflichtmodule", &[(5, C)]),
    (3, "Wahlpflichtmodule", &[(2, E)]),
    (2, "Bautechnik und Ökologie", &[]),
    (3, "Pflichtmodule", &[(5, C)]),
    (3, "Wahlpflichtmodule", &[(5, E)]),
    (2, "Geschichte und Theorie", &[]),
    (3, "Pflichtmodule", &[(4, C)]),
    (3, "Wahlpflichtmodule", &[(8, E)]),
];

/// Wirtschaftsingenieurwesen B.Sc. dual, PO 2023 (`bachelor-wirtschaftsingenieurwesen-dual-2023`):
/// one field with 22 areas below it, in five fields of its own.
pub const WIRTSCHAFTSINGENIEURWESEN_DUAL: &[Node] = &[
    (1, "Mathematisch-Methodischer Bereich", &[(7, U), (1, C)]),
    (1, "Wirtschaftswissenschaftlicher Bereich", &[]),
    (2, "Pflichtmodule", &[(6, C)]),
    (2, "Wahlpflicht Wirtschaftswissenschaften", &[]),
    (3, "Finanzierung, Finanzmärkte und Unternehmensrechnung", &[(6, E)]),
    (3, "Innovation und Marketing", &[(11, E)]),
    (3, "Unternehmensentwicklung und Marktstrukturen", &[(8, E)]),
    (1, "Ingenieurwissenschaftlicher Schwerpunkt", &[]),
    (2, "Produktionstechnik", &[]),
    (3, "Pflichtbereich Produktionstechnik", &[(5, C)]),
    (3, "Wahlpflicht Produktionstechnik", &[(1, E)]),
    (4, "Technische Produktkonzeption", &[(3, C), (7, E)]),
    (4, "Industrialisierung", &[(8, E)]),
    (4, "Digitale Produktion", &[(7, E)]),
    (2, "Umwelttechnik", &[]),
    (3, "Pflichtbereich Umwelttechnik", &[]),
    (4, "Pflichtmodule", &[(6, C)]),
    (4, "Wahlpflichtmodule", &[(2, E)]),
    (3, "Wahlpflicht Umwelttechnik", &[(1, E)]),
    (4, "Kreislauf und Entsorgung", &[(10, E)]),
    (4, "Wassertechnik", &[(8, E)]),
    (2, "Energiesysteme", &[]),
    (3, "Pflichtbereich Energiesysteme", &[(5, C)]),
    (3, "Wahlpflicht Energiesysteme", &[(2, E)]),
    (4, "Elektrische Energietechnik", &[(9, E)]),
    (4, "Energiewirtschaft", &[(4, E)]),
    (4, "Thermische Energietechnik", &[(1, C), (5, E)]),
    (2, "Bauingenieurwesen", &[]),
    (3, "Wahlpflicht Bauingenieurwesen", &[(1, E)]),
    (4, "Mechanik, Statik, Dynamik", &[(5, E)]),
    (4, "Material, Tragwerk, Konstruktion", &[(7, E)]),
    (4, "Gebäude, Stadt, Umwelt", &[(7, E)]),
    (4, "Wirtschaft, Recht, Management", &[(4, E)]),
    (4, "Projekte", &[(4, E)]),
    (2, "Elektro- und Informationstechnik", &[]),
    (3, "Pflichtbereich Elektro- und Informationssysteme", &[(7, C)]),
    (3, "Wahlpflicht Elektro- und Informationssysteme", &[(1, C), (3, E)]),
    (4, "Informations- und Kommunikationstechnik", &[(7, E)]),
    (4, "Medientechnik", &[(3, E)]),
    (4, "Elektronik und Messtechnik", &[(4, E)]),
    (4, "Hochfrequenztechnik", &[(5, E)]),
    (1, "Praxisintegrierendes Studium", &[(3, U)]),
];

/// A row of a plan that names no module, as `v_program_plan_entry` has it.
pub fn row(name: &str, semester: i64, kind: Option<ModuleKind>, plan: Option<&str>) -> PlanEntry {
    PlanEntry {
        module_id: None,
        module_name: name.to_string(),
        semester: Some(semester),
        start_semester: None,
        end_semester: None,
        semester_span: None,
        credits: Some(6.0),
        min_credits: None,
        max_credits: None,
        kind: kind.map(Code::Known),
        kind_raw: None,
        study_section: None,
        subject_area: None,
        specialization: plan.map(str::to_string),
        catalog_title: None,
        credits_differ_from_catalog: false,
    }
}

/// The rows of the plan of Informatik B.Sc. that name no module.
pub fn informatik_bsc_rows() -> Vec<PlanEntry> {
    [
        ("Proseminar oder Praktikum", 3),
        ("Modul aus dem Bereich Praktische Mathematik", 3),
        ("Anwendungsfach", 3),
        ("Fachübergreifendes Studium", 4),
        ("Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik", 4),
        ("Komplex Grundlagen der Informatik", 5),
        ("Komplex Praktische Informatik", 5),
        ("Komplex Angewandte und Technische Informatik", 5),
        ("Seminar oder Praktikum", 6),
    ]
    .into_iter()
    .map(|(name, semester)| row(name, semester, E, None))
    .collect()
}
