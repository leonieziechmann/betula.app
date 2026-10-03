//! The chips and choices of the catalog's filters, also on the start page's board: a filter
//! value off, wanted or unwanted (`Tri`, `Toggle`), one of a few (`Choice`).

use std::sync::Arc;

use folia_model::labels::{Campus, Labelled, OfferStatus, TeachingForm, TurnusParity};
use folia_routes::filter::{
    CatalogQuery, ExamPart, Language, TurnusFilter,
};
use leptos::prelude::*;

use crate::i18n::{self, Locale};


/// A filter value is off, wanted, or unwanted („keine Vorträge").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tri {
    Off,
    With,
    Without,
}

impl Tri {
    /// The chip's `data-state`, which the stylesheet draws.
    pub fn code(self) -> &'static str {
        match self {
            Tri::Off => "off",
            Tri::With => "with",
            Tri::Without => "without",
        }
    }

    /// The chip's `aria-checked`: an unwanted value is „mixed".
    pub fn checked(self) -> &'static str {
        match self {
            Tri::Off => "false",
            Tri::With => "true",
            Tri::Without => "mixed",
        }
    }
}

pub type ReadTri = dyn Fn(&CatalogQuery) -> Tri + Send + Sync;

pub type WriteTri = dyn Fn(&mut CatalogQuery, Tri) + Send + Sync;

pub type Held = dyn Fn(&CatalogQuery) -> bool + Send + Sync;

/// How a chip reads its state from the filter and writes it back. The board of the filters on the
/// start page (`home::detail`) switches its chips with the same toggles.
#[derive(Clone)]
pub struct Toggle {
    pub read: Arc<ReadTri>,
    pub write: Arc<WriteTri>,
    /// Off → with → without → off. Otherwise only off ↔ with.
    pub excludes: bool,
    /// When the filter is such that the chip has to stay as it is (the last class the finder
    /// compares), and why: it is no link then.
    pub held: Option<(Arc<Held>, &'static str)>,
}

impl Toggle {
    pub fn new(read: impl Fn(&CatalogQuery) -> Tri + Send + Sync + 'static, write: impl Fn(&mut CatalogQuery, Tri) + Send + Sync + 'static) -> Self {
        Self { read: Arc::new(read), write: Arc::new(write), excludes: true, held: None }
    }

    /// A value that is wanted when in the first list and unwanted when in the second.
    fn in_lists<T: PartialEq + Copy + Send + Sync + 'static>(
        value: T,
        lists: fn(&CatalogQuery) -> (&Vec<T>, &Vec<T>),
        lists_mut: fn(&mut CatalogQuery) -> (&mut Vec<T>, &mut Vec<T>),
    ) -> Self {
        Self::new(
            move |q| {
                let (with, without) = lists(q);
                if with.contains(&value) {
                    Tri::With
                } else if without.contains(&value) {
                    Tri::Without
                } else {
                    Tri::Off
                }
            },
            move |q, state| {
                let (with, without) = lists_mut(q);
                with.retain(|v| *v != value);
                without.retain(|v| *v != value);
                match state {
                    Tri::With => with.push(value),
                    Tri::Without => without.push(value),
                    Tri::Off => {}
                }
            },
        )
    }

    pub fn teaching_form(form: TeachingForm) -> Self {
        Self::in_lists(form, |q| (&q.teaching_forms, &q.teaching_forms_exclude), |q| (&mut q.teaching_forms, &mut q.teaching_forms_exclude))
    }

    pub fn exam_part(part: ExamPart) -> Self {
        Self::in_lists(part, |q| (&q.exam_parts, &q.exam_parts_exclude), |q| (&mut q.exam_parts, &mut q.exam_parts_exclude))
    }

    pub fn language(language: Language) -> Self {
        Self::in_lists(language, |q| (&q.languages, &q.languages_exclude), |q| (&mut q.languages, &mut q.languages_exclude))
    }

    pub fn campus(campus: Campus) -> Self {
        Self::in_lists(campus, |q| (&q.campuses, &q.campuses_exclude), |q| (&mut q.campuses, &mut q.campuses_exclude))
    }

    /// „Only such modules" / „no such modules" on a yes-no property.
    pub fn flag(get: fn(&CatalogQuery) -> Option<bool>, set: fn(&mut CatalogQuery, Option<bool>)) -> Self {
        Self::new(
            move |q| match get(q) {
                Some(true) => Tri::With,
                Some(false) => Tri::Without,
                None => Tri::Off,
            },
            move |q, state| {
                set(
                    q,
                    match state {
                        Tri::With => Some(true),
                        Tri::Without => Some(false),
                        Tri::Off => None,
                    },
                )
            },
        )
    }

    /// A season of the turnus (winter, summer, irregular): wanted in the first of its two fields,
    /// unwanted in the second.
    pub fn turnus(fields: fn(&TurnusFilter) -> (bool, bool), fields_mut: fn(&mut TurnusFilter) -> (&mut bool, &mut bool)) -> Self {
        Self::new(
            move |q| match fields(&q.turnus) {
                (true, _) => Tri::With,
                (_, true) => Tri::Without,
                _ => Tri::Off,
            },
            move |q, state| {
                let (with, without) = fields_mut(&mut q.turnus);
                (*with, *without) = (state == Tri::With, state == Tri::Without);
            },
        )
    }

    /// „Auch nicht angebotene zeigen": every offer status instead of the default ones.
    pub fn show_not_offered() -> Self {
        Self {
            excludes: false,
            ..Self::new(
                |q| if q.offer.as_ref().is_some_and(|offer| offer.contains(&OfferStatus::NotOffered)) { Tri::With } else { Tri::Off },
                |q, state| q.offer = (state == Tri::With).then(|| OfferStatus::ALL.to_vec()),
            )
        }
    }

    pub fn after(&self, state: Tri) -> Tri {
        match state {
            Tri::Off => Tri::With,
            Tri::With if self.excludes => Tri::Without,
            Tri::With | Tri::Without => Tri::Off,
        }
    }
}

/// One of a few: a row of links that fills the width, the chosen one raised.
pub struct Choice {
    pub label: String,
    pub title: Option<&'static str>,
    pub count: Option<Signal<Option<u64>>>,
    pub is_on: Arc<dyn Fn(&CatalogQuery) -> bool + Send + Sync>,
    pub choose: Arc<dyn Fn(&mut CatalogQuery) + Send + Sync>,
}

impl Choice {
    pub fn new(
        label: impl Into<String>,
        is_on: impl Fn(&CatalogQuery) -> bool + Send + Sync + 'static,
        choose: impl Fn(&mut CatalogQuery) + Send + Sync + 'static,
    ) -> Self {
        Self { label: label.into(), title: None, count: None, is_on: Arc::new(is_on), choose: Arc::new(choose) }
    }
}

/// „Dauer": any, one semester, two.
pub fn duration_choices(locale: Locale) -> Vec<Choice> {
    let t = i18n::texts(locale);
    vec![
        Choice::new(t.catalog.any, |q| q.duration_semesters.is_none(), |q| q.duration_semesters = None),
        Choice::new((t.catalog.semesters)(1), |q| q.duration_semesters == Some(1), |q| q.duration_semesters = Some(1)),
        Choice::new((t.catalog.semesters)(2), |q| q.duration_semesters == Some(2), |q| q.duration_semesters = Some(2)),
    ]
}

/// „Nur in geraden / ungeraden Jahren": any, even, odd.
pub fn years_choices(locale: Locale) -> Vec<Choice> {
    let t = i18n::texts(locale);
    vec![
        Choice::new(t.catalog.any, |q| q.turnus.year_parity.is_none(), |q| q.turnus.year_parity = None),
        Choice::new(t.catalog.even, |q| q.turnus.year_parity == Some(TurnusParity::Even), |q| q.turnus.year_parity = Some(TurnusParity::Even)),
        Choice::new(t.catalog.odd, |q| q.turnus.year_parity == Some(TurnusParity::Odd), |q| q.turnus.year_parity = Some(TurnusParity::Odd)),
    ]
}
