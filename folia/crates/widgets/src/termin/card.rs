//! The card of one Termin (owner, 2026-11-17: „Das was gerade auch noch nicht gute läuft ist, dass
//! wenn man auf einen termin klickt, dass dann infos kommen die halt nicht nur für den Termin sind.
//! Das muss auch angepasst werden so dass das mehr wie bei google calendar"): what a click on a
//! Termin of the Stundenplan or of the start page opens, in both of the Stundenplan's states.
//!
//! From top to bottom: the head — the module's chip in its tone, the kind, „Besuch" or „von dir",
//! and the tools, which never depend on the date shown („Ändern" or „Bearbeiten", the eye
//! „Ausblenden", „⋯" with its menu, „×"; of a Termin of one's own the bin „Löschen") —, the module's
//! title, the series in one line, the ticket with its date arrows (owner, 2026-10-27: „die Tasten
//! ← → fürs Datum"), what is about the date picked there (a clash, why QIS cancels it, where it
//! moved, „Am … ausgeblendet · Einblenden", „Von dir geändert … Zurücksetzen"), who only where the
//! Termin names somebody, QIS's remark and the note, of a group its state and the one large action,
//! and one way on, „Alle Termine von MIT1" (V), the overview with the module's events, groups and
//! „Reihe" (`…&full=1`).
//!
//! The eye hides, the bin deletes, „×" only closes (owner, 2026-11-17: „Ich will übrigens dass wir
//! vom x weg kommen und eher ein augen symbol verwneden" — „da wo es nur ausblendet das auge und da
//! wo es entfernt eine Mülltonne"). What takes the Termin out of the week or decides it closes the
//! card, and the page answers (`TerminCtx::done`: a note with „Rückgängig" while the plan is looked
//! at, the slot itself while it is edited). „Ausblenden", „Ändern" and the note's field take the
//! card's body; Esc leads back to the card, one layer at a time (`CardState::peel`).
//!
//! On a wide screen the card stands beside the slot's day column it was opened from, its top at the
//! slot's where the window lets it (`card_place`), and keeps one edge and its width whatever it
//! shows: its top, growing downward, or low in the window its bottom, growing upward; on a phone it
//! is a sheet. Nothing here runs on the server: `/event/<key>` looks a Termin up (R9).

use folia_calendar::semester::SemesterKey;
use folia_design::modal::{Edge, Placed};
use folia_design::nav::Room;
use folia_design::waitnote::WaitNote;
use folia_plans::studyplan::{PlanDoc, PlanUndo};
use folia_stores::collections::Anchor;
use folia_timetable::choice;
use folia_timetable::model::TakenBy;

use super::*;

/// What the card of one Termin says and does beside its ticket.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CardPart {
    /// „Besuch": its module is visited, not studied.
    pub visit: bool,
    /// The series in one line, which no arrow changes: „wöchentlich · 15 Termine · noch 8"; of one
    /// of one's own „Jede Woche · 05.10.–31.01., die Vorlesungszeit"; none of a single date.
    pub series: Option<String>,
    /// The dates of it the student hid, named after the series („ausgeblendet: Mo 23.11."): each
    /// leads the ticket there.
    pub off_dates: Vec<(String, Day)>,
    /// How many more are hidden than are named.
    pub off_more: usize,
    /// An extra beside the module's compulsory part („zusätzlich, freiwillig"), as its slot says.
    pub extra: bool,
    /// The plan leaves the Termin out: why, in a line under the series and above the ticket
    /// („Ausgeblendet", „Vorlesung ausgeblendet", „Andere Gruppe gewählt · Mo 15:30").
    pub off: Option<String>,
    /// The lines of the date shown, under the ticket: a clash, why QIS cancels it, where it moved,
    /// the room's note.
    pub of_date: Vec<CardLine>,
    /// The date shown is hidden: „Am Mo 23.11. ausgeblendet", and what shows it again.
    pub date_off: Option<(String, CardAct)>,
    /// Of a Termin of one's own on a date it skips: the eye's words („Am Do 19.11. einblenden") and
    /// what shows the date again.
    pub own_show: Option<(String, CardAct)>,
    /// Who leads this Termin, where QIS names somebody at it (the module's responsible is the
    /// overview's).
    pub who: Vec<Person>,
    /// QIS's remark at the Termin.
    pub remark: Option<String>,
    /// The student's note.
    pub note: Option<String>,
    /// Of a group of a choice the plan shows: its state („Eine von 4 Übungsgruppen · noch keine
    /// angehakt", „Angehakt · 1 von 4 Übungsgruppen") and, of one ticked, what takes its tick away.
    pub group: Option<(String, Option<CardAct>)>,
    /// The one large action, and the one beside it („Auch diese").
    pub big: Option<BigAct>,
    pub also: Option<BigAct>,
    /// „Vergleichen ›": the code of its event's choice of groups (`choice::code`).
    pub compare: Option<String>,
    /// What „Ausblenden" offers, the narrowest first; of a Termin of one's own the one thing its
    /// eye does at once; empty where there is nothing to hide.
    pub hide: Vec<HideChoice>,
    /// „Ändern": a Termin of QIS with a time that the plan holds.
    pub change: bool,
    /// The way on, „Alle Termine von MIT1"; none of a Termin of one's own.
    pub all: Option<String>,
    /// „In QIS öffnen".
    pub qis: Option<String>,
}

/// A line of the date: what it says; the date of the Termin it leads to („Verlegt auf Do 26.11.",
/// where that day is one of its dates); and whether it is the line of a date QIS cancels, on which
/// „Nachholtermin anlegen ›" stands.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CardLine {
    pub hint: Hint,
    pub to: Option<Day>,
    pub makeup: bool,
}

/// A large action of the card: its words, what it does, its `data-action`, and what the page's
/// note says of it (`None`: the week itself answers, nothing is said).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct BigAct {
    pub label: String,
    pub act: CardAct,
    pub name: &'static str,
    pub said: Option<String>,
}

/// A row of „Ausblenden": its words, the line under them, its sign, what it does, its
/// `data-action`, and what the page's note says once it is done.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct HideChoice {
    pub label: String,
    pub sub: String,
    pub icon: &'static str,
    pub act: CardAct,
    pub name: &'static str,
    pub said: String,
}

/// What a button of the card does to the plan — what a slot's buttons do (the planner's
/// `studyplan::week::Act`, which it keeps for its slots; no layer below the planner has it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CardAct {
    /// One date hidden or shown again (`set_date`); of a Termin of one's own (`skip_own`).
    Date { key: RowKey, day: Day, hide: bool },
    OwnDate { number: u32, day: Day, hide: bool },
    /// The Termin as a whole hidden: every key of it (`set_row`).
    Rows(Vec<RowKey>),
    /// A ticked group left out: its tick taken away, its Termine hidden.
    Drop { ticks: Vec<RowKey>, rows: Vec<RowKey> },
    /// The whole event hidden (`set_event`); `Leave`: one the visitor took among its module's
    /// events, taken back and hidden.
    Event(u32),
    Leave(u32),
    /// Shown again: its event where that is hidden, a kind, its Termine.
    Show { event: Option<u32>, kind: Option<EventKind>, rows: Vec<RowKey> },
    /// Its group ticked instead of what was („Diese Gruppe anhaken" of an open choice, „Nur diese
    /// Gruppe"), or beside those ticked („Auch diese"); either way shown again where it was hidden.
    Only { event: u32, row: RowKey, rows: Vec<RowKey> },
    Tick { event: u32, row: RowKey, rows: Vec<RowKey> },
    Untick(Vec<RowKey>),
    /// Its event, an option among its module's events, taken: instead of `others`, or beside what
    /// is taken.
    Take { others: Vec<u32>, event: u32, row: RowKey, rows: Vec<RowKey> },
}

impl CardAct {
    /// Makes the change in the plan, for the semester `sem`, as the slots' buttons make it.
    pub(super) fn apply(&self, doc: &mut PlanDoc, sem: SemesterKey) {
        match self {
            CardAct::Date { key, day, hide } => doc.set_date(sem, *key, *day, *hide),
            CardAct::OwnDate { number, day, hide } => {
                doc.skip_own(*number, *day, *hide);
            }
            CardAct::Rows(rows) => rows.iter().for_each(|row| doc.set_row(sem, *row, true)),
            CardAct::Drop { ticks, rows } => {
                ticks.iter().for_each(|row| doc.tick(sem, *row, false));
                rows.iter().for_each(|row| doc.set_row(sem, *row, true));
            }
            CardAct::Event(event) => doc.set_event(sem, *event, true),
            CardAct::Leave(event) => {
                doc.choose(sem, *event, None);
                doc.set_event(sem, *event, true);
            }
            CardAct::Show { event, kind, rows } => {
                if let Some(event) = event {
                    doc.set_event(sem, *event, false);
                }
                if let Some(kind) = kind {
                    doc.set_kind(sem, *kind, false);
                }
                rows.iter().for_each(|row| doc.set_row(sem, *row, false));
            }
            CardAct::Only { event, row, rows } => {
                doc.set_event(sem, *event, false);
                rows.iter().for_each(|row| doc.set_row(sem, *row, false));
                doc.choose(sem, *event, Some(*row));
            }
            CardAct::Tick { event, row, rows } => {
                doc.set_event(sem, *event, false);
                rows.iter().for_each(|row| doc.set_row(sem, *row, false));
                doc.tick(sem, *row, true);
            }
            CardAct::Untick(ticks) => ticks.iter().for_each(|row| doc.tick(sem, *row, false)),
            CardAct::Take { others, event, row, rows } => {
                others.iter().for_each(|other| doc.choose(sem, *other, None));
                doc.set_event(sem, *event, false);
                rows.iter().for_each(|row| doc.set_row(sem, *row, false));
                doc.choose(sem, *event, Some(*row));
            }
        }
    }
}

/// What `card_part` works from: what `shown_of` found of the Termin.
pub(super) struct Facts<'a> {
    pub table: &'a Timetable,
    pub o: &'a Occasion,
    pub event: &'a Event,
    pub modules: &'a BTreeMap<String, ModuleFacts>,
    pub today: Option<Day>,
    pub t: &'static Texts,
    pub head: &'a Head,
    pub first_kind: Option<EventKind>,
    /// Who leads it, where QIS names somebody at the Termin.
    pub who: Vec<Person>,
    /// What the date meets, and QIS's remark.
    pub clash: Option<Hint>,
    pub remark: Option<String>,
    pub groups: Option<&'a Groups>,
    pub plan: Option<&'a PlanPart>,
    pub own: Option<&'a OwnPart>,
    pub change: Option<&'a ChangePart>,
    pub qis: Option<&'a str>,
}

/// The card of the Termin `f` found, in its seven cases: a firm Termin, a group of an
/// open choice, a group ticked, one the plan leaves out, one of one's own, a date QIS cancels, a
/// Termin changed by the student.
pub(super) fn card_part(f: Facts<'_>) -> CardPart {
    let t = f.t;
    let words = &t.termin;
    let locale = t.locale;
    let (o, event) = (f.o, f.event);
    let id = event.id.parse::<u32>().ok();
    let keys = row_keys(event, o);
    let first = o.rows.first().and_then(|row| event.rows.get(*row));
    let rhythm = rhythm_word(first, t);
    let when = first_when(event, o, locale);
    let what = called(event, f.modules, t);
    let chip = f.head.chip.as_str();
    // The kind in a sentence of Betula's own: in German QIS's word as the head has it („Vorlesung
    // ganz"); in another language the kinds' labels („The whole lecture"), QIS's word as written
    // where the event has no kind Betula knows — never lower-cased, it is a name.
    let labels: Vec<&str> = event.kinds.iter().map(|kind| kind.label(locale)).collect();
    let own_word = locale != Locale::De && !labels.is_empty();
    let kind = if own_word { labels.join("/") } else if f.head.kind.is_empty() { words.panel.to_string() } else { f.head.kind.clone() };
    let within = if own_word { kind.to_lowercase() } else { kind.clone() };
    let kind = kind.as_str();
    let n = o.place(None).1;
    let sessions = termin::sessions(event);
    let tile = f.groups.and_then(|groups| groups.tiles.iter().find(|tile| tile.current));
    let all = f.groups.map_or(0, |groups| groups.tiles.len());
    let ticked = f.groups.map_or(0, |groups| groups.tiles.iter().filter(|tile| tile.ticked).count());
    let group_word = (words.groups)(f.first_kind);
    // The choice among the module's events the event is an option of, and the ids of its events.
    let across = event.across.and_then(|alt| Some((alt, f.table.choices.get(alt.choice)?)));
    let every: Vec<u32> = across.map(|(_, across)| across.options.iter().flatten().filter_map(|index| f.table.events.get(*index)?.id.parse().ok()).collect()).unwrap_or_default();

    let series = match f.own {
        Some(own) => Some((words.card_own_series)(&own.rhythm, &own.when)),
        None if o.dates.len() < 2 => None,
        None => {
            let (_, count, left) = o.place(f.today);
            Some((words.card_series)(&rhythm, &(words.dates)(count), &(words.left)(left)))
        }
    };
    // The dates hidden, named once: the one shown says so under the ticket. Three by name, the
    // rest as a number („… +2").
    let hidden_days: Vec<Day> = o.dates.iter().filter(|dated| dated.state == DateState::Hidden && dated.day != o.day).map(|dated| dated.day).collect();
    let off_dates = hidden_days.iter().take(3).map(|day| (day_words(*day, f.today, locale), *day)).collect();
    let off_more = hidden_days.len().saturating_sub(3);

    let mut of_date: Vec<CardLine> = f.clash.into_iter().map(|hint| CardLine { hint, to: None, makeup: false }).collect();
    match &o.state {
        DateState::Cancelled(reason, instead) => {
            let reason = reason.trim();
            let text = if reason.is_empty() { words.cancelled.to_string() } else { reason.to_string() };
            of_date.push(CardLine { hint: Hint { warn: true, icon: "calendar-minus", text }, to: None, makeup: true });
            if let Some(day) = instead {
                let to = o.dates.iter().any(|dated| dated.day == *day).then_some(*day);
                of_date.push(CardLine { hint: Hint { warn: false, icon: "arrow-right", text: (words.moved_to)(&day_words(*day, f.today, locale)) }, to, makeup: false });
            }
        }
        DateState::Note(note) => of_date.push(CardLine { hint: Hint { warn: false, icon: "map-pin", text: note.clone() }, to: None, makeup: false }),
        _ => {}
    }
    let date_off = match (f.plan.and_then(|plan| plan.date.clone()), f.own) {
        (Some((said, key, day, true)), _) => Some(((words.card_date_off)(&said), CardAct::Date { key, day, hide: false })),
        (_, Some(own)) => match &own.date {
            Some((said, day, true)) => Some(((words.card_date_off)(said), CardAct::OwnDate { number: own.number, day: *day, hide: false })),
            _ => None,
        },
        _ => None,
    };

    // A group, the plan's answer to it, and what brings back a Termin the plan leaves out.
    let ticking = |tile: &TileShown| Some((words.card_said_ticked)(&what, &tile.when));
    let (mut group, mut big, mut also, mut off) = (None, None, None, None);
    match o.hidden {
        Some(HiddenBy::Row) => {
            off = Some(words.hidden_event.to_string());
            big = Some(BigAct { label: words.card_show.to_string(), act: CardAct::Show { event: None, kind: None, rows: keys.clone() }, name: "show-termin", said: None });
        }
        Some(HiddenBy::Event) => {
            off = Some((words.card_event_off)(kind));
            big = Some(BigAct { label: (words.card_show_event)(&within), act: CardAct::Show { event: id, kind: None, rows: keys.clone() }, name: "show-event", said: None });
        }
        Some(HiddenBy::Kinds) => {
            // It brings back every event of the kind, which its words say.
            let label = f.first_kind.map_or("", |kind| kind.label(locale));
            off = Some((words.kind_off)(label));
            big = Some(BigAct { label: (words.card_show_kind)(label), act: CardAct::Show { event: None, kind: f.first_kind, rows: keys.clone() }, name: "show-kind", said: None });
        }
        Some(HiddenBy::Choice) => {
            let chosen: Vec<&str> = f.groups.map(|groups| groups.tiles.iter().filter(|tile| tile.ticked).map(|tile| tile.when.as_str()).collect()).unwrap_or_default();
            off = Some((words.card_other_group)(&chosen.join(" · ")));
            if let (Some(id), Some(tile)) = (id, tile) {
                if let Some(row) = tile.keys.first().copied() {
                    big = Some(BigAct { label: words.card_only_group.to_string(), act: CardAct::Only { event: id, row, rows: tile.keys.clone() }, name: "tick-only", said: ticking(tile) });
                    also = Some(BigAct { label: words.card_also.to_string(), act: CardAct::Tick { event: id, row, rows: tile.keys.clone() }, name: "tick-also", said: ticking(tile) });
                }
            }
        }
        Some(HiddenBy::Option) => {
            off = Some(words.other_option.to_string());
            if let Some(id) = id {
                let said = Some((words.card_said_ticked)(&what, &when));
                big = Some(BigAct { label: words.card_only.to_string(), act: CardAct::Take { others: every.clone(), event: id, row: o.key, rows: keys.clone() }, name: "take-only", said: said.clone() });
                also = Some(BigAct { label: words.card_also.to_string(), act: CardAct::Take { others: Vec::new(), event: id, row: o.key, rows: keys.clone() }, name: "take-also", said });
            }
        }
        Some(HiddenBy::Program) => {
            off = Some(words.other_programs.to_string());
            if let Some(id) = id {
                big = Some(BigAct { label: words.card_take.to_string(), act: CardAct::Take { others: every.clone(), event: id, row: o.key, rows: keys.clone() }, name: "take", said: Some((words.card_said_ticked)(&what, &when)) });
            }
        }
        // Another course of the module is not shown from here.
        Some(HiddenBy::Town(_) | HiddenBy::Course(_)) => off = Some(words.other_place.to_string()),
        None => match (f.groups, tile, id) {
            (Some(_), Some(tile), Some(id)) if ticked == 0 => {
                group = Some((format!("{} · {}", (words.card_one_of)(all, group_word), words.card_none_ticked), None));
                if let Some(row) = tile.keys.first().copied() {
                    big = Some(BigAct { label: words.card_tick.to_string(), act: CardAct::Only { event: id, row, rows: tile.keys.clone() }, name: "tick", said: ticking(tile) });
                }
            }
            (Some(_), Some(tile), _) if tile.ticked => group = Some(((words.card_ticked_of)(ticked, all, group_word), Some(CardAct::Untick(tile.keys.clone())))),
            (_, _, Some(id)) if across.is_some_and(|(alt, _)| alt.open) => {
                big = Some(BigAct { label: words.card_take.to_string(), act: CardAct::Take { others: Vec::new(), event: id, row: o.key, rows: keys.clone() }, name: "take", said: Some((words.card_said_ticked)(&what, &when)) });
            }
            _ => {}
        },
    }
    // „Vergleichen ›" where the Termin is a group of a choice, not beside a required Termin of an
    // event that has groups besides.
    let compare = (f.groups.is_some() && tile.is_some() && event.hidden.is_none()).then(|| choice::code(event));

    // What „Ausblenden" offers, the narrowest first; nothing of a Termin the plan leaves out.
    let mut hide = Vec::new();
    match (o.hidden, f.own) {
        (Some(_), _) => {}
        // A Termin of one's own is not hidden as a whole but deleted: its eye hides the date.
        (None, Some(own)) => {
            if let Some((said, day, _)) = &own.date {
                // A name the student wrote stands in quotes, one composed of kind and module not („VL EvS").
                let named = event.own.as_ref().is_some_and(|own| !own.name.trim().is_empty());
                let note = if named { (words.card_said_own_date)(&what, said) } else { (words.card_said_date)(&what, said) };
                hide.push(HideChoice { label: (words.card_hide_own)(said), sub: String::new(), icon: "eye", act: CardAct::OwnDate { number: own.number, day: *day, hide: true }, name: "hide-date", said: note });
            }
        }
        (None, None) => {
            let in_group = f.groups.is_some() && tile.is_some();
            // A group of an open choice: the plan does not hold its date yet.
            let open_group = in_group && tile.is_some_and(|tile| !tile.ticked);
            if let Some((said, key, day, false)) = f.plan.and_then(|plan| plan.date.clone()).filter(|_| !open_group) {
                hide.push(HideChoice { label: (words.only_on)(&said), sub: words.others_stay.to_string(), icon: "calendar-minus", act: CardAct::Date { key, day, hide: true }, name: "hide-date", said: (words.card_said_date)(&what, &said) });
            }
            if in_group || sessions.len() > 1 {
                let (label, sub, act) = match tile.filter(|_| in_group) {
                    // A Termin of one date is „Diesen Termin", not „Alle 1".
                    None if n < 2 => (words.card_this_termin.to_string(), (words.card_when_rhythm)(&when, &rhythm), CardAct::Rows(keys.clone())),
                    None => ((words.change_all)(n), (words.card_when_rhythm)(&when, &rhythm), CardAct::Rows(keys.clone())),
                    // The others stand to choose again only where this one was the one ticked.
                    Some(tile) if tile.ticked => {
                        let sub = if ticked > 1 { (words.card_others_stay_ticked)(ticked - 1) } else { (words.card_others_open_again)(all.saturating_sub(1)) };
                        (words.card_hide_group.to_string(), sub, CardAct::Drop { ticks: tile.keys.clone(), rows: keys.clone() })
                    }
                    Some(_) => (words.card_hide_group.to_string(), (words.card_others_to_choose)(all.saturating_sub(1)), CardAct::Rows(keys.clone())),
                };
                hide.push(HideChoice { label, sub, icon: "repeat", act, name: "hide-termin", said: (words.card_said_termin)(&what, &when) });
            }
            if let Some(id) = id {
                let sub = match (in_group, sessions.len() > 1) {
                    (true, _) => (words.card_all_groups)(all),
                    (false, true) => {
                        let others: Vec<String> = sessions.iter().filter(|session| !keys.contains(&session.key)).filter_map(|session| event.rows.get(session.row)).map(|row| when_of(row, locale)).collect();
                        (words.card_also_at)(&others.join(" · "))
                    }
                    (false, false) => (words.card_all_dates)(n),
                };
                // An option the visitor took among the module's events is taken back as it goes.
                let leave = across.is_some_and(|(alt, across)| across.by == Some(TakenBy::Chosen) && across.taken.contains(&alt.option));
                let act = if leave { CardAct::Leave(id) } else { CardAct::Event(id) };
                hide.push(HideChoice { label: (words.card_hide_whole)(&within), sub, icon: "layers", act, name: "hide-event", said: (words.card_said_event)(kind, chip) });
            }
        }
    }

    CardPart {
        visit: event.visit,
        series,
        off_dates,
        off_more,
        extra: event.extra && f.own.is_none(),
        off,
        of_date,
        own_show: f.own.and_then(|own| match &own.date {
            Some((said, day, true)) => Some(((words.card_show_own)(said), CardAct::OwnDate { number: own.number, day: *day, hide: false })),
            _ => None,
        }),
        date_off,
        who: f.who,
        remark: f.remark,
        note: f.change.and_then(|change| change.note.clone()),
        group,
        big,
        also,
        compare,
        hide,
        change: f.change.is_some() && o.hidden.is_none() && f.plan.is_some_and(|plan| plan.shown && plan.held),
        all: f.own.is_none().then(|| (words.card_all)(chip)),
        qis: f.qis.filter(|_| f.own.is_none()).map(str::to_string),
    }
}

// ---------- where it stands ----------

/// The card's width as the stylesheet has it (app.css `.st-dialog.tm-card-modal`).
pub(super) const WIDTH: f64 = 400.0;
const GAP: f64 = 10.0;
const EDGE: f64 = 16.0;
/// The bodies that take the card's place below its head, as tall as they are drawn (app.css, „The
/// card of one Termin"; measured 319, 171, 94 + 52 a row), each with a little room for a line that
/// wraps: „Ändern" with the line of what the change meets, the note's field, and „Ausblenden" by
/// its rows.
const BODY_CHANGE: f64 = 345.0;
const BODY_NOTE: f64 = 190.0;
const BODY_HIDE: f64 = 104.0;
const HIDE_ROW: f64 = 54.0;

/// How tall the bodies of a card can be that take its place under its head (`CardPart`): „Ändern"
/// where it has the pencil, „Ausblenden" by its rows where the eye opens a choice, the note's field
/// where „⋯" offers it. 0 for a card that has none of them: a Termin the plan leaves out (no
/// pencil, no eye), a Termin of one's own (its pencil is the form over the page, its eye acts at
/// once).
pub(super) fn tallest_body(card: &CardPart, own: bool, note: bool) -> f64 {
    if own {
        return 0.0;
    }
    let change = if card.change { BODY_CHANGE } else { 0.0 };
    let hide = if card.hide.is_empty() { 0.0 } else { BODY_HIDE + HIDE_ROW * card.hide.len() as f64 };
    let note = if note { BODY_NOTE } else { 0.0 };
    change.max(hide).max(note)
}

/// What the card measures of itself as it rests (nothing open in it): its height, and its body's
/// below the head. The bodies of „Ändern", „Ausblenden" and the note take the body's place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Size {
    pub rest: f64,
    pub body: f64,
    /// `tallest_body`.
    pub tallest: f64,
}

impl Size {
    /// As tall as it can grow: its head and the tallest of its bodies, or as it rests.
    pub(super) fn most(self) -> f64 {
        (self.rest - self.body + self.tallest).max(self.rest)
    }
}

/// Where the card stands (`Placed`). `at`: the slot's day column from left to right, the slot from
/// top to bottom. Beside the column on the side with more room — the other side where it fits
/// there alone —, its top at the slot's; where neither side has room for it, at the window's edge
/// and under the slot, else over it: never on the slot while the window leaves another place.
/// Always inside the window and never under the rail (`Room`). Without a slot in the middle of the
/// page, 12 % of the window down.
///
/// One edge stays still whatever the card shows (owner, 2026-10-16: „Modale menus springen zu viel
/// in der größe"): where it can grow as tall as it can under the slot's top, its top stands at the
/// slot's and it grows downward; else its bottom stands where the resting card ends beside the
/// slot — the top still at the slot's — and it grows upward, and where even the resting card does
/// not fit under the slot's top, its bottom is 16 px over the window's edge. No button lies under
/// the window's edge, nothing makes the window scroll.
pub(super) fn card_place(at: Option<Anchor>, width: f64, room: Room, size: Size) -> Placed {
    let Room { left: rail, width: view, height: tall } = room;
    let least = rail + EDGE;
    let most = (view - width - EDGE).max(least);
    let bottom = |y: f64| Edge::Bottom((tall - y).max(EDGE));
    let grown = size.most();
    let top_at = |top: f64| match top + grown <= tall - EDGE {
        true => Edge::Top(top),
        false => bottom((top + size.rest).min(tall - EDGE)),
    };
    let Some(at) = at else {
        let left = (rail + (view - rail - width) / 2.0).clamp(least, most);
        return Placed { left, edge: top_at((tall * 0.12).max(EDGE)) };
    };
    let (right, left) = (at.right + GAP, at.left - GAP - width);
    let (fits_right, fits_left) = (right <= most, left >= least);
    let more_right = view - at.right >= at.left - rail;
    let x = match (more_right, fits_right, fits_left) {
        (true, true, _) | (false, true, false) => right,
        (false, _, true) | (true, false, true) => left,
        (true, false, false) => most,
        (false, false, false) => least,
    };
    let beside = x >= at.right || x + width <= at.left;
    let edge = match beside {
        true => top_at(at.top.max(EDGE)),
        // Under the slot where the resting card fits there, else over it, its bottom above it.
        false if at.bottom + GAP + size.rest <= tall - EDGE => top_at(at.bottom + GAP),
        false => bottom((at.top - GAP).max(EDGE + size.rest).min(tall - EDGE)),
    };
    Placed { left: x, edge }
}

/// How tall the card in the open dialog is as drawn, and its body (what scrolls when it is cut).
pub(super) fn measured() -> Option<(f64, f64)> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let dialog = folia_design::stage::select("dialog.tm-card-modal[open]")?.dyn_into::<web_sys::HtmlElement>().ok()?;
        let body = dialog.query_selector(".tm-card > .dbody").ok().flatten()?.dyn_into::<web_sys::HtmlElement>().ok()?;
        let (shown, scroll) = (f64::from(body.client_height()), f64::from(body.scroll_height()));
        let tall = f64::from(dialog.offset_height()) - shown + scroll;
        (tall > 0.0).then_some((tall, scroll))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The card's dialog, while it is open on the page in front.
#[cfg(feature = "csr")]
fn dialog() -> Option<web_sys::HtmlElement> {
    use wasm_bindgen::JsCast;
    folia_design::stage::select("dialog.tm-card-modal[open]").and_then(|dialog| dialog.dyn_into::<web_sys::HtmlElement>().ok())
}

/// Whether the keyboard is nowhere in the card: on the dialog itself or fallen to the page
/// (`body`), as when what had it went.
#[cfg(feature = "csr")]
fn lost(dialog: &web_sys::HtmlElement) -> bool {
    let at = leptos::prelude::document().active_element();
    at.is_none_or(|at| {
        let own: &web_sys::Element = dialog.as_ref();
        at == *own || !own.contains(Some(&at))
    })
}

/// Gives the keyboard to what `selector` finds in the card, where the keyboard is nowhere in it.
fn focus_in(selector: &str) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(dialog) = dialog() else { return };
        if !lost(&dialog) {
            return;
        }
        match dialog.query_selector(selector).ok().flatten().and_then(|to| to.dyn_into::<web_sys::HtmlElement>().ok()) {
            Some(to) => {
                let _ = to.focus();
            }
            None => {
                let _ = dialog.focus();
            }
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = selector;
}

/// The large action has the keyboard as the card opens (a click, then Enter), where there is one
/// and the keyboard is nowhere in the card yet: nothing a visitor reached for loses it.
pub(super) fn focus_big() {
    focus_in(".tm-act .tm-big");
}

// ---------- the view ----------

/// What stands open inside the card: „Ändern" with what it holds, „Ausblenden"'s choice or the
/// note's field — one of them at a time, in the card's body — and the menu of „⋯" over it. The
/// dialog owns it, so that Esc closes what is open in the card before the card (`peel`), and a
/// card that closed leaves nothing open for the next (`reset`).
#[derive(Clone, Copy)]
pub(super) struct CardState {
    editing: RwSignal<Option<ChangeDraft>>,
    inner: RwSignal<Inner>,
    more: RwSignal<bool>,
    /// The card did what closes it: it shows what it showed until it has gone.
    leaving: RwSignal<bool>,
    /// What has the keyboard once the layer open in the card goes (Esc, „Abbrechen", „Speichern"):
    /// the tool that opened it, a selector in the card.
    returns: StoredValue<Option<&'static str>>,
    /// How tall the bodies can be that take the card's place (`tallest_body`), for where it stands.
    pub(super) tallest: RwSignal<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Inner {
    #[default]
    None,
    Hide,
    Note,
}

impl CardState {
    pub(super) fn new() -> Self {
        CardState {
            editing: RwSignal::new(None),
            inner: RwSignal::new(Inner::None),
            more: RwSignal::new(false),
            leaving: RwSignal::new(false),
            returns: StoredValue::new(None),
            tallest: RwSignal::new(0.0),
        }
    }

    /// Something stands open in the card: a body or the menu.
    pub(super) fn layered(&self) -> bool {
        self.editing.try_with_untracked(Option::is_some).unwrap_or(false) || self.inner.try_get_untracked().is_some_and(|inner| inner != Inner::None) || self.more.try_get_untracked().unwrap_or(false)
    }

    /// Esc: closes what is open in the card — the menu first, else the body —; `false` where
    /// nothing is (the card closes).
    pub(super) fn peel(&self) -> bool {
        if self.more.try_get_untracked().unwrap_or(false) {
            let _ = self.more.try_set(false);
            return true;
        }
        let open = self.layered();
        if open {
            self.editing.set(None);
            self.inner.set(Inner::None);
        }
        open
    }

    /// Opens a body or the menu, from the tool `from` (a selector), which has the keyboard again
    /// once it goes.
    fn returns_to(&self, from: &'static str) {
        self.returns.set_value(Some(from));
    }

    /// The keyboard back where it belongs once what had it went: the tool that opened the layer
    /// just closed, else the large action, else the dialog.
    pub(super) fn refocus(&self) {
        let to = self.returns.try_get_value().flatten();
        let _ = self.returns.try_set_value(None);
        request_animation_frame(move || match to {
            Some(to) => focus_in(to),
            None => focus_big(),
        });
    }

    /// Nothing open: the card closed, or another Termin's card stands in its place.
    pub(super) fn reset(&self) {
        let _ = self.editing.try_set(None);
        let _ = self.inner.try_set(Inner::None);
        let _ = self.more.try_set(false);
        let _ = self.leaving.try_set(false);
        let _ = self.returns.try_set_value(None);
    }
}

/// Gives the keyboard to what `selector` finds in the card, a frame later, wherever it is now: a
/// field or a menu that has just opened.
fn focus_now(selector: &'static str) {
    request_animation_frame(move || {
        #[cfg(feature = "csr")]
        {
            use wasm_bindgen::JsCast;
            if let Some(to) = dialog().and_then(|dialog| dialog.query_selector(selector).ok().flatten()).and_then(|to| to.dyn_into::<web_sys::HtmlElement>().ok()) {
                let _ = to.focus();
            }
        }
        #[cfg(not(feature = "csr"))]
        let _ = selector;
    });
}

/// The card of the Termin `ctx.asked`, top to bottom: `section.is-termin.tm-dialog.tm-card`.
#[component]
pub(super) fn TerminCard(ctx: TerminCtx, closing: Callback<()>, state: CardState) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let live = shown_memo(ctx);
    // On its way out the card shows what it showed: the plan changes under it after the frame
    // (`run`), and the dialog goes when the address says so, which can be frames later.
    let shown = Memo::new(move |before: Option<&Option<Shown>>| match (state.leaving.get(), before) {
        (true, Some(before)) => before.clone(),
        _ => live.get(),
    });
    // Each part of `shown` alone (R16: never `ctx.table` and `shown` in one closure).
    let found = Memo::new(move |_| shown.with(Option::is_some));
    let loading = Memo::new(move |_| ctx.table.with(Option::is_none));
    let sem = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map(|table| table.key)));
    let head = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| (s.head.clone(), s.pick.title.clone()))));
    let card = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.card.clone())));
    let own = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.own.clone())));
    let change = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.change.clone())));
    let at = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| (s.key, s.day))));
    let called = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| format!("{} · {} · {}", s.head.kind, s.head.title, s.ticket.date)).unwrap_or_default()));
    let changing = Memo::new(move |_| state.editing.with(|draft| draft.as_ref().map(|draft| (draft.key, draft.day))).is_some_and(|open| Some(open) == at.get()));

    // How tall its bodies can be, for where the dialog stands.
    Effect::new(move |_| {
        let tallest = card.with(|card| card.as_ref().map(|card| tallest_body(card, own.with(Option::is_some), change.with(Option::is_some)))).unwrap_or(0.0);
        state.tallest.set(tallest);
    });
    // The keyboard never falls out of the card: once a layer closed (Esc, „Abbrechen",
    // „Speichern") it goes back to the tool that opened it, and once the card changed under it
    // („Haken wegnehmen", „Einblenden", „Zurücksetzen") to what stands in place of what had it.
    let layer = Memo::new(move |_| state.editing.with(Option::is_some) || state.inner.get() != Inner::None || state.more.get());
    Effect::new(move |before: Option<bool>| {
        let open = layer.get();
        if before == Some(true) && !open {
            state.refocus();
        }
        open
    });
    Effect::new(move |before: Option<()>| {
        shown.track();
        if before.is_some() && !layer.get_untracked() {
            state.refocus();
        }
    });

    // What takes the Termin out of the week or decides it: the card closes as its × does, the page
    // answers (its note with „Rückgängig", or the slot itself), and the plan follows after the
    // frame — as „Löschen" of a Termin of one's own does. „Rückgängig" takes back what this did,
    // not what was done meanwhile elsewhere (`PlanUndo`).
    let run = Callback::new(move |(act, said): (CardAct, Option<String>)| {
        let (Some(plan), Some(sem)) = (ctx.plan, sem.get_untracked()) else { return };
        match ctx.done {
            Some(done) => {
                if let Some(said) = said {
                    let undo = PlanUndo::made(plan.with_untracked(Clone::clone), |doc| act.apply(doc, sem));
                    done.run(TerminUndo { said, undo });
                }
                state.leaving.set(true);
                closing.run(());
                plan.update_after_paint(move |doc| act.apply(doc, sem));
            }
            // A page that answers nothing: the card stays and shows what it did.
            None => plan.update(|doc| act.apply(doc, sem)),
        }
    });
    // What the card answers itself: the group's line, the date's line.
    let stay = Callback::new(move |act: CardAct| {
        let (Some(plan), Some(sem)) = (ctx.plan, sem.get_untracked()) else { return };
        plan.update(|doc| act.apply(doc, sem));
    });
    // A click in the card beside the menu of „⋯" closes it.
    let beside_menu = move |ev: leptos::ev::MouseEvent| {
        if !state.more.get_untracked() {
            return;
        }
        #[cfg(feature = "csr")]
        {
            use wasm_bindgen::JsCast;
            let inside = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).and_then(|at| at.closest(".tm-menu, [data-action=\"more\"]").ok().flatten()).is_some();
            if !inside {
                state.more.set(false);
            }
        }
        #[cfg(not(feature = "csr"))]
        let _ = ev;
    };

    let close = move || view! { <button class="ghost hit dlg-close tm-close" type="button" aria-label=t.common.close title=words.card_close_title on:click=move |_| closing.run(())><Icon name="x"/></button> };
    let top = move || {
        let Some((head, sub)) = head.get() else {
            return view! { <div class="hero-top tm-bar"><div class="tm-tools">{close()}</div></div> }.into_any();
        };
        let chip = view! { <span class=format!("tm-mod {}", hue(head.tone)) class:words=head.words id="tm-chip"><i></i>{head.chip.clone()}</span> };
        let kind = (!head.kind.is_empty()).then(|| view! { <span class="tm-kind" id="tm-kind">{head.kind.clone()}</span> });
        let tags = move || {
            let visit = card.with(|card| card.as_ref().is_some_and(|card| card.visit));
            // „von dir" of one of one's own that has a module: without one its chip says it.
            let mine = own.with(Option::is_some) && !head.words;
            view! {
                {visit.then(|| view! { <span class="np tm-visit">{words.card_visit}</span> })}
                {mine.then(|| view! { <span class="np tm-own">{words.card_own}</span> })}
            }
        };
        view! {
            <div class="hero-top tm-bar">
                {chip}
                {kind}
                {tags}
                <div class="tm-tools">
                    <CardTools ctx card own change at called state run stay/>
                    {close()}
                </div>
            </div>
            <h2 class="tm-title" id=TITLE_ID>{head.title}</h2>
            {sub.map(|sub| view! { <p class="tm-sub">{sub}</p> })}
            <CardSeries ctx card at/>
        }
        .into_any()
    };

    let body = move || {
        if !found.get() {
            return match loading.get() {
                true => ().into_any(),
                false => view! { <p class="hint">{words.gone}</p> }.into_any(),
            };
        }
        if changing.get() {
            return view! { <ChangeView ctx shown editing=state.editing/> }.into_any();
        }
        match state.inner.get() {
            Inner::Hide => view! { <HideView card state run/> }.into_any(),
            Inner::Note => match change.get_untracked() {
                Some(part) => {
                    let removable = part.note.is_some();
                    view! {
                        <div class="tm-note-body">
                            <h4>{words.note}<span class="r">{words.note_sub}</span></h4>
                            <NoteField ctx key=part.key note=part.note removable on_done=Callback::new(move |()| state.inner.set(Inner::None))/>
                        </div>
                    }
                    .into_any()
                }
                None => ().into_any(),
            },
            Inner::None => view! {
                <TicketView ctx shown/>
                <CardDateLines ctx card at stay/>
                <ChangedView ctx shown/>
                <CardPeople card change sem state/>
                <CardAction ctx card state run stay/>
                <CardWays ctx card own at/>
            }
            .into_any(),
        }
    };

    view! {
        <section class="is-termin tm-dialog tm-card" aria-label=words.panel on:click=beside_menu>
            <header class="hero tm-head">{top}</header>
            <div class="dbody tm st-dlg-body" data-sheet-body="">{body}</div>
        </section>
    }
}

/// The head's tools: „Ändern" (of one's own „Bearbeiten"), the eye „Ausblenden", of one's own the
/// bin „Löschen", „⋯" with its menu. Which stand comes from the Termin, never from the date shown
/// (an arrow in the ticket would else change what stands above it: CLAUDE.md, „What picks stands
/// above what it changes") — but the eye of a Termin of one's own, which says what is (owner,
/// 2026-11-17: „da wo es nur ausblendet das auge"): on a date already hidden it is the crossed eye
/// and shows the date again.
#[component]
fn CardTools(
    ctx: TerminCtx,
    card: Memo<Option<CardPart>>,
    own: Memo<Option<OwnPart>>,
    change: Memo<Option<ChangePart>>,
    at: Memo<Option<(RowKey, Day)>>,
    called: Memo<String>,
    state: CardState,
    run: Callback<(CardAct, Option<String>)>,
    stay: Callback<CardAct>,
) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let tools = Memo::new(move |_| card.with(|card| card.as_ref().map(|card| (card.change, card.hide.first().cloned(), !card.hide.is_empty(), card.own_show.clone()))));
    let number = Memo::new(move |_| own.with(|own| own.as_ref().map(|own| own.number)));
    let expanded = |on: bool| if on { "true" } else { "false" };
    let editing = Memo::new(move |_| state.editing.with(Option::is_some));
    move || {
        let (can_change, first_hide, any_hide, own_show) = tools.get()?;
        let number = number.get();
        let pencil = match (number, ctx.own) {
            (Some(number), Some(acts)) => Some(view! {
                <button class="ghost tm-tool" type="button" data-action="own-edit" title=words.own_edit_title aria-label=words.own_edit on:click=move |_| acts.edit.run(number)><Icon name="pencil"/></button>
            }.into_any()),
            (None, _) if can_change => Some(view! {
                <button class="ghost tm-tool" type="button" data-action="change" title=words.card_change_title aria-label=words.change aria-expanded=move || expanded(editing.get())
                    on:click=move |_| {
                        state.more.set(false);
                        // A second press closes it, as the eye's does; what was typed goes with it.
                        if state.editing.with_untracked(Option::is_some) {
                            state.editing.set(None);
                            return;
                        }
                        let Some(part) = change.get_untracked() else { return };
                        state.returns_to("[data-action=\"change\"]");
                        state.inner.set(Inner::None);
                        state.editing.set(Some(ChangeDraft::of(&part)));
                    }>
                    <Icon name="pencil"/>
                </button>
            }.into_any()),
            _ => None,
        };
        let eye = match (number, first_hide, own_show) {
            // Of one's own, on a date already hidden: the crossed eye shows it again, the card stays.
            (Some(_), _, Some((label, act))) => Some(view! {
                <button class="ghost tm-tool" type="button" data-action="show-date" title=label.clone() aria-label=label on:click=move |_| stay.run(act.clone())>
                    <Icon name="eye-off"/>
                </button>
            }.into_any()),
            // Of one's own the eye hides the date at once (it has one reach).
            (Some(_), Some(choice), None) => Some(view! {
                <button class="ghost tm-tool" type="button" data-action="hide-date" title=choice.label.clone() aria-label=choice.label.clone() on:click=move |_| run.run((choice.act.clone(), Some(choice.said.clone())))>
                    <Icon name="eye"/>
                </button>
            }.into_any()),
            (None, _, _) if any_hide => Some(view! {
                <button class="ghost tm-tool" type="button" data-action="hide" title=words.hide aria-label=words.hide aria-expanded=move || expanded(state.inner.get() == Inner::Hide)
                    on:click=move |_| {
                        state.more.set(false);
                        state.editing.set(None);
                        state.returns_to("[data-action=\"hide\"]");
                        state.inner.update(|inner| *inner = if *inner == Inner::Hide { Inner::None } else { Inner::Hide });
                    }>
                    <Icon name="eye"/>
                </button>
            }.into_any()),
            _ => None,
        };
        let bin = match (number, ctx.own) {
            (Some(number), Some(acts)) => Some(view! {
                <button class="ghost tm-tool danger" type="button" data-action="own-remove" title=words.own_remove aria-label=words.own_remove on:click=move |_| acts.remove.run(number)><Icon name="trash-2"/></button>
            }),
            _ => None,
        };
        let more = number.is_none().then(|| view! {
            <button class="ghost tm-tool" type="button" data-action="more" title=words.card_more aria-label=words.card_more_title aria-haspopup="menu" aria-expanded=move || expanded(state.more.get())
                on:click=move |_| {
                    state.returns_to("[data-action=\"more\"]");
                    state.more.update(|more| *more = !*more);
                    if state.more.get_untracked() {
                        focus_now(".tm-menu [role=menuitem]");
                    }
                }>
                <Icon name="ellipsis"/>
            </button>
            {move || state.more.get().then(|| view! { <CardMenu ctx card change at called state/> })}
        });
        Some(view! { {pencil}{eye}{bin}{more} })
    }
}

/// The menu of „⋯" under its sign, over the card's body, which it never makes taller: „Notiz
/// hinzufügen" (or „bearbeiten"), „Nachholtermin anlegen", „Teilen", „In QIS öffnen ↗" — each entry
/// one line and a button as a whole. Esc and a click beside it close it.
#[component]
fn CardMenu(ctx: TerminCtx, card: Memo<Option<CardPart>>, change: Memo<Option<ChangePart>>, at: Memo<Option<(RowKey, Day)>>, called: Memo<String>, state: CardState) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let qis = card.with_untracked(|card| card.as_ref().and_then(|card| card.qis.clone()));
    let part = change.with_untracked(|change| change.as_ref().map(|change| (change.key, change.day, change.note.is_some())));
    let note = part.map(|(_, _, has)| {
        view! {
            <button class="tm-menu-row" type="button" role="menuitem" data-action="note"
                on:click=move |_| {
                    state.more.set(false);
                    state.editing.set(None);
                    state.inner.set(Inner::Note);
                    focus_now("#tm-note-field");
                }>
                <Icon name="file-text"/><span>{if has { words.card_note_edit } else { words.card_note_add }}</span>
            </button>
        }
    });
    let makeup = part.zip(ctx.change).map(|((key, day, _), acts)| {
        view! {
            <button class="tm-menu-row" type="button" role="menuitem" data-action="makeup" on:click=move |_| { state.more.set(false); acts.makeup.run((key, day)); }>
                <Icon name="calendar-plus"/><span>{words.card_makeup}</span>
            </button>
        }
    });
    let href = move || at.get().map(|(key, day)| t.path(&url::event_path(key, Some(day)))).unwrap_or_default();
    // „Teilen" says „Kopiert" in its place (enhance.js `share-link`): the menu stays for it.
    let share = view! {
        <button class="tm-menu-row tm-share js-only" type="button" role="menuitem" data-action="share-link" data-text=href data-absolute="" data-title=move || called.get() title=words.share_title>
            <Icon name="share-2"/><span data-label="">{words.share}</span>
        </button>
    };
    let qis = qis.map(|href| {
        view! {
            <a class="tm-menu-row" role="menuitem" href=href rel="noopener" target="_blank" on:click=move |_| state.more.set(false)>
                <Icon name="arrow-up-right"/><span>{words.open_in_qis}" ↗"</span>
            </a>
        }
    });
    view! { <div class="tm-menu" role="menu" aria-label=words.card_more_title>{note}{makeup}{share}{qis}</div> }
}

/// The series in one line (which no arrow changes), the other dates of it hidden as links to them
/// (three by name, the rest as a number), whether it is an extra, and why the plan leaves the
/// Termin out (cases d1, d2), above the ticket.
#[component]
fn CardSeries(ctx: TerminCtx, card: Memo<Option<CardPart>>, at: Memo<Option<(RowKey, Day)>>) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let lines = Memo::new(move |_| card.with(|card| card.as_ref().map(|card| (card.series.clone(), card.off_dates.clone(), card.off_more, card.extra, card.off.clone()))));
    let key = Memo::new(move |_| at.get().map(|(key, _)| key));
    move || {
        let (series, off_dates, off_more, extra, off) = lines.get()?;
        let key = key.get()?;
        let series = match (series, extra) {
            (Some(series), true) => Some(format!("{series} · {}", words.card_extra)),
            (None, true) => Some(words.card_extra.to_string()),
            (series, false) => series,
        };
        let dates = (!off_dates.is_empty()).then(|| {
            let links = off_dates
                .into_iter()
                .enumerate()
                .map(|(at, (said, day))| {
                    view! {
                        {(at > 0).then_some(", ")}
                        <a class="tm-off-date" href=ctx.links.termin.run((key, Some(day))) rel="nofollow" data-noscroll="" data-pick="">{said}</a>
                    }
                })
                .collect_view();
            let head = (words.card_series_off)("");
            let more = (off_more > 0).then(|| format!(" {}", (words.card_more_off)(off_more)));
            view! { {series.is_some().then_some(" · ")}{head}{links}{more} }
        });
        let when = (series.is_some() || dates.is_some()).then(|| view! { <p class="tm-when">{series}{dates}</p> });
        let off = off.map(|off| view! { <p class="tm-off"><Icon name="eye-off"/><span>{off}</span></p> });
        Some(view! { {when}{off} })
    }
}

/// Under the ticket, what is about the date it picks: a clash, why QIS cancels it (with
/// „Nachholtermin anlegen ›"), where it moved (a link where that is one of its dates), the room's
/// note; and „Am Mo 23.11. ausgeblendet" with „Einblenden", which the card answers itself.
#[component]
fn CardDateLines(ctx: TerminCtx, card: Memo<Option<CardPart>>, at: Memo<Option<(RowKey, Day)>>, stay: Callback<CardAct>) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let lines = Memo::new(move |_| card.with(|card| card.as_ref().map(|card| (card.of_date.clone(), card.date_off.clone()))));
    move || {
        let (of_date, date_off) = lines.get()?;
        let (key, day) = at.get()?;
        if of_date.is_empty() && date_off.is_none() {
            return None;
        }
        let lines = of_date
            .into_iter()
            .map(|line| {
                let text = match line.to {
                    Some(to) => view! { <a href=ctx.links.termin.run((key, Some(to))) rel="nofollow" data-noscroll="" data-pick="">{line.hint.text}</a> }.into_any(),
                    None => line.hint.text.into_any(),
                };
                let makeup = ctx.change.filter(|_| line.makeup).map(|acts| {
                    view! { <button class="link tm-makeup" type="button" data-action="makeup" on:click=move |_| acts.makeup.run((key, day))>{words.card_makeup}<Icon name="chevron-right"/></button> }
                });
                view! { <p class="tm-hint" class:no=line.hint.warn><Icon name=line.hint.icon/><span>{text}{makeup}</span></p> }
            })
            .collect_view();
        let off = date_off.map(|(said, act)| {
            view! {
                <p class="tm-hint tm-date-off">
                    <Icon name="eye-off"/>
                    <span>{said}</span>
                    <button class="sw" type="button" data-action="show-date" on:click=move |_| stay.run(act.clone())>{words.card_show}</button>
                </p>
            }
        });
        Some(view! { <div class="tm-lines">{lines}{off}</div> })
    }
}

/// Who leads the Termin where it names somebody, QIS's remark, and the note with „Bearbeiten";
/// under it the hint of Betula Sync where another device wrote into the note at the same time.
#[component]
fn CardPeople(card: Memo<Option<CardPart>>, change: Memo<Option<ChangePart>>, sem: Memo<Option<SemesterKey>>, state: CardState) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let lines = Memo::new(move |_| card.with(|card| card.as_ref().map(|card| (card.who.clone(), card.remark.clone(), card.note.clone()))));
    let key = Memo::new(move |_| change.with(|change| change.as_ref().map(|change| change.key)));
    move || {
        let (who, remark, note) = lines.get()?;
        let conflict = key.get().map(|row| view! { <crate::syncnote::TerminNoteConflict semester=Signal::derive(move || sem.get()) row/> });
        if who.is_empty() && remark.is_none() && note.is_none() {
            return conflict.map(IntoAny::into_any);
        }
        let people = who.into_iter().map(|person| view! { <div class="who"><span class="av"><Icon name="user-round"/></span><span><b>{person.name}</b><small>{person.role}</small></span></div> }).collect_view();
        let remark = remark.map(|remark| view! { <p class="tm-hint"><Icon name="info"/><span>{remark}</span></p> });
        let note = note.map(|note| {
            view! {
                <p class="tm-hint tm-note-line">
                    <Icon name="file-text"/>
                    <span>{words.note}": "<span class="tm-note-text">{note}</span></span>
                    <button class="sw" type="button" data-action="note-edit"
                        on:click=move |_| {
                            state.returns_to("[data-action=\"note-edit\"]");
                            state.editing.set(None);
                            state.inner.set(Inner::Note);
                            focus_now("#tm-note-field");
                        }>
                        {words.note_edit}
                    </button>
                </p>
            }
        });
        Some(view! { <div class="tm-lines">{people}{remark}{note}</div>{conflict} }.into_any())
    }
}

/// Of a group its state and the one large action („Diese Gruppe anhaken", „Nur diese Gruppe" ·
/// „Auch diese", „Einblenden" …), „Vergleichen ›"; of a group ticked, under its line, „Haken
/// wegnehmen" at the left and „Vergleichen ›" at the right, where the large action and „Vergleichen
/// ›" stand of one not ticked.
#[component]
fn CardAction(ctx: TerminCtx, card: Memo<Option<CardPart>>, state: CardState, run: Callback<(CardAct, Option<String>)>, stay: Callback<CardAct>) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let parts = Memo::new(move |_| card.with(|card| card.as_ref().map(|card| (card.group.clone(), card.big.clone(), card.also.clone(), card.compare.clone()))));
    move || {
        let (group, big, also, compare) = parts.get()?;
        let compare = compare.zip(ctx.links.choice).map(|(code, choice)| choice.run(code));
        if group.is_none() && big.is_none() && compare.is_none() {
            return None;
        }
        let (group, untick) = match group {
            Some((said, untick)) => (Some(view! { <p class="tm-group"><span>{said}</span></p> }), untick),
            None => (None, None),
        };
        // The tick away: the large action comes back, and has the keyboard.
        let untick = untick.map(|act| {
            view! {
                <button class="sw tm-untick" type="button" data-action="tick" aria-pressed="true" on:click=move |_| { state.returns_to(".tm-act .tm-big"); stay.run(act.clone()); }>{words.card_untick}</button>
            }
        });
        let button = move |act: BigAct, class: &'static str| {
            let pressed = (act.name == "tick").then_some("false");
            let BigAct { label, act, name, said } = act;
            view! { <button class=class type="button" data-action=name aria-pressed=pressed on:click=move |_| run.run((act.clone(), said.clone()))>{label}</button> }
        };
        let big = big.map(|act| button(act, "tm-big"));
        let also = also.map(|act| button(act, "tm-also mini"));
        let compare = compare.map(|href| view! { <a class="go2" href=href data-action="compare" data-replace="" data-noscroll="">{words.card_compare}<Icon name="chevron-right"/></a> });
        Some(view! { <div class="tm-act">{group}{big}{also}{untick}{compare}</div> })
    }
}

/// The one way on: „Alle Termine von MIT1" (V), the overview in the card's place in the history;
/// of a Termin of one's own that it reaches no file and no fixed subscription, and from the start
/// page the way to change it in the Stundenplan.
#[component]
fn CardWays(ctx: TerminCtx, card: Memo<Option<CardPart>>, own: Memo<Option<OwnPart>>, at: Memo<Option<(RowKey, Day)>>) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let all = Memo::new(move |_| card.with(|card| card.as_ref().and_then(|card| card.all.clone())));
    let mine = Memo::new(move |_| own.with(Option::is_some));
    move || {
        let (key, day) = at.get()?;
        let all = all.get().zip(ctx.links.full).map(|(said, full)| {
            // Its name without the cap „V", which `aria-keyshortcuts` says.
            let name = said.clone();
            view! {
                <a class="tm-all" href=full.run((key, Some(day))) rel="nofollow" data-action="fullscreen" data-replace="" data-noscroll="" aria-keyshortcuts="V" aria-label=name>
                    <span>{said}</span><Icon name="chevron-right"/><Shortcut keys="V"/>
                </a>
            }
        });
        let feed = mine.get().then(|| view! { <p class="tm-own-feed">{words.own_feed}</p> });
        let change = (mine.get() && ctx.own.is_none()).then(|| {
            let href = t.path(&url::StudyplanUrl::default().with_termin(Some((key, Some(day)))).path());
            view! { <a class="tm-way" href=href>{words.own_change}<Icon name="chevron-right"/></a> }
        });
        (all.is_some() || feed.is_some() || change.is_some()).then(|| view! { <nav class="tm-ways">{all}{feed}{change}</nav> })
    }
}

/// „Ausblenden", the card's body while it is open: its reaches, the narrowest first, each saying
/// what it hides; the first has the keyboard as it comes. „Abbrechen" (and Esc) leads back, the
/// keyboard to the eye.
#[component]
fn HideView(card: Memo<Option<CardPart>>, state: CardState, run: Callback<(CardAct, Option<String>)>) -> impl IntoView {
    let t = i18n::t();
    let words = &t.termin;
    let first = NodeRef::<leptos::html::Button>::new();
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        if let Some(first) = first.get() {
            let _ = first.focus();
        }
    });
    let choices = card.with_untracked(|card| card.as_ref().map(|card| card.hide.clone()).unwrap_or_default());
    let rows = choices
        .into_iter()
        .enumerate()
        .map(|(at, choice)| {
            let HideChoice { label, sub, icon, act, name, said } = choice;
            let row = view! {
                <span class="ci"><Icon name=icon/></span>
                <span><b>{label}</b><small>{sub}</small></span>
            };
            let click = move |_| run.run((act.clone(), Some(said.clone())));
            match at {
                0 => view! { <button class="tm-hide-row" type="button" data-action=name node_ref=first on:click=click>{row}</button> }.into_any(),
                _ => view! { <button class="tm-hide-row" type="button" data-action=name on:click=click>{row}</button> }.into_any(),
            }
        })
        .collect_view();
    view! {
        <div class="tm-hide">
            <h4>{words.hide}<span class="r">{words.in_feed}</span></h4>
            {rows}
            <div class="chg-foot">
                <button class="mini" type="button" data-action="hide-cancel" on:click=move |_| state.inner.set(Inner::None)>{words.cancel}</button>
            </div>
        </div>
    }
}

/// The note of what a Termin's card did, with „Rückgängig", for a page that has none of its own
/// (the start page): the sentence, and what the card did taken back (`PlanUndo`) — in the profile
/// it was done in: with another profile chosen (here or in another tab) there is nothing to undo.
#[component]
pub fn TerminNote(plan: Option<Studyplan>, undo: RwSignal<Option<TerminUndo>>) -> impl IntoView {
    let t = i18n::t();
    let said = Memo::new(move |_| undo.with(|undo| undo.as_ref().map(|undo| undo.said.clone())));
    Effect::new(move |before: Option<u16>| {
        let profile = plan.map_or(0, Studyplan::profile);
        if before.is_some_and(|before| before != profile) {
            undo.set(None);
        }
        profile
    });
    let restore = move |_| {
        let (Some(plan), Some(TerminUndo { undo: done, .. })) = (plan, undo.get_untracked()) else { return };
        undo.set(None);
        plan.update_after_paint(move |doc| done.restore(doc));
    };
    move || {
        said.get().map(|said| {
            // A note that leaves lets go of itself alone, not of a newer one that took its place
            // (the next card's, within the 200 ms the old one takes to go).
            let mine = said.clone();
            let gone = Callback::new(move |()| {
                if undo.with_untracked(|now| now.as_ref().is_some_and(|now| now.said == mine)) {
                    undo.set(None);
                }
            });
            view! {
                <WaitNote on_gone=gone class="planning-note own-note">
                    <div class="plan-toast out">
                        <div class="plan-toast-main">
                            <span class="plan-toast-mark" aria-hidden="true"><Icon name="check"/></span>
                            <span class="plan-toast-said"><span class="plan-toast-said-name">{said}</span></span>
                        </div>
                        <div class="plan-toast-end">
                            <span class="plan-toast-sep" aria-hidden="true"></span>
                            <button class="plan-toast-undo" type="button" on:click=restore>{t.common.undo}</button>
                        </div>
                    </div>
                </WaitNote>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use folia_calendar::own::{OwnRhythm, OwnTermin};
    use folia_calendar::select::Selection;
    use folia_timetable::model::tests::{d, table, teaching};

    use super::*;

    fn modules() -> BTreeMap<String, ModuleFacts> {
        [("12104".to_string(), ModuleFacts { title: "Entwicklung von Softwaresystemen".into(), abbrev: Some("EvS".into()), credits: Some(6.0), responsible: Some("Köhler, Ekkehard".into()) })].into_iter().collect()
    }

    fn card_of(t: &Timetable, key: RowKey, on: Option<&str>) -> CardPart {
        card_in(t, key, on, &i18n::DE)
    }

    fn card_in(t: &Timetable, key: RowKey, on: Option<&str>, texts: &'static Texts) -> CardPart {
        shown_of(t, &Against::Itself, key, on.map(d), Some((d("2026-11-10"), 8 * 60)), &modules(), (TerminMode::Adjust, Some(true)), texts).unwrap().card.unwrap()
    }

    fn names(card: &CardPart) -> Vec<&'static str> {
        card.hide.iter().map(|choice| choice.name).collect()
    }

    #[test]
    fn the_card_names_its_series_and_the_dates_hidden() {
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45")];
        let key = rows[0].key();
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let card = card_of(&t, key, Some("2026-11-10"));
        assert_eq!(card.series.as_deref(), Some("wöchentlich · 15 Termine · noch 10"));
        assert!(card.off_dates.is_empty() && card.date_off.is_none());
        let hidden = table(&rows, &["12104"], &[], &Selection { hidden_dates: [(key, d("2026-11-17"))].into_iter().collect(), ..Selection::default() });
        // On the hidden date the line under the ticket says so, and the series' line does not again.
        let card = card_of(&hidden, key, Some("2026-11-17"));
        assert!(card.off_dates.is_empty());
        assert_eq!(card.date_off.as_ref().map(|(said, _)| said.as_str()), Some("Am Di 17.11. ausgeblendet"));
        assert!(matches!(card.date_off, Some((_, CardAct::Date { hide: false, .. }))));
        // On another date of it the series' line names it and the line does not stand; the head's
        // tools are the same.
        let other = card_of(&hidden, key, Some("2026-11-24"));
        assert!(other.date_off.is_none());
        assert_eq!(other.off_dates, [("Di 17.11.".to_string(), d("2026-11-17"))]);
        assert_eq!((other.hide.is_empty(), other.change), (card.hide.is_empty(), card.change));
        // Five hidden: three by name, two as a number.
        let days = ["2026-11-03", "2026-11-17", "2026-11-24", "2026-12-01", "2026-12-08"];
        let five = table(&rows, &["12104"], &[], &Selection { hidden_dates: days.iter().map(|day| (key, d(day))).collect(), ..Selection::default() });
        let card = card_of(&five, key, Some("2026-11-10"));
        assert_eq!((card.off_dates.len(), card.off_more), (3, 2));
    }

    #[test]
    fn ausblenden_offers_the_date_the_termin_and_the_event() {
        // A lecture held twice a week: the date, the Termin, the event.
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45"), teaching("12104", "100", 2, "Vorlesung", 5, "09:15", "10:45")];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let card = card_of(&t, rows[0].key(), Some("2026-11-10"));
        assert_eq!(names(&card), ["hide-date", "hide-termin", "hide-event"]);
        assert!(matches!(card.hide[0].act, CardAct::Date { hide: true, .. }) && matches!(card.hide[1].act, CardAct::Rows(_)) && matches!(card.hide[2].act, CardAct::Event(100)));
        assert_eq!(card.hide.iter().map(|choice| choice.label.as_str()).collect::<Vec<_>>(), ["Nur am Di 10.11.", "Alle 15", "Vorlesung ganz"]);
        assert_eq!(card.hide.iter().map(|choice| choice.sub.as_str()).collect::<Vec<_>>(), ["die anderen Termine bleiben", "Di 09:15 · wöchentlich", "auch Fr 09:15"]);
        assert_eq!(card.hide[0].said, "VL EvS am Di 10.11. ausgeblendet");
        assert_eq!(card.hide[1].said, "VL EvS Di 09:15 ausgeblendet");
        assert_eq!(card.hide[2].said, "Vorlesung EvS ausgeblendet");
        assert!(card.change && card.big.is_none() && card.group.is_none() && card.all.as_deref() == Some("Alle Termine von EvS"));
        // A lecture of one Termin: the date and the event, „alle 15".
        let one = table(&rows[..1], &["12104"], &[], &Selection::default());
        let card = card_of(&one, rows[0].key(), Some("2026-11-10"));
        assert_eq!(names(&card), ["hide-date", "hide-event"]);
        assert_eq!(card.hide[1].sub, "alle 15");
    }

    #[test]
    fn a_group_of_an_open_choice_is_ticked_from_its_card() {
        let rows = vec![
            teaching("12104", "200", 1, "Übung", 1, "11:30", "13:00").group("1-Gruppe"),
            teaching("12104", "200", 2, "Übung", 2, "09:15", "10:45").group("2-Gruppe"),
            teaching("12104", "200", 3, "Übung", 3, "11:30", "13:00").group("3-Gruppe"),
        ];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let card = card_of(&t, rows[0].key(), None);
        assert_eq!(card.group, Some(("Eine von 3 Übungsgruppen · noch keine angehakt".to_string(), None)));
        let big = card.big.clone().unwrap();
        assert!(big.name == "tick" && big.label == "Diese Gruppe anhaken" && matches!(big.act, CardAct::Only { event: 200, .. }));
        assert_eq!(big.said.as_deref(), Some("Ü EvS Mo 11:30 angehakt"));
        assert_eq!(card.compare.as_deref(), Some("e200"));
        assert_eq!(names(&card), ["hide-termin", "hide-event"]);
        assert_eq!(card.hide[0].sub, "die anderen 2 bleiben zur Wahl");
        assert_eq!(card.hide[1].sub, "alle 3 Gruppen");
        assert!(card.change);
    }

    #[test]
    fn a_ticked_group_gives_its_tick_back() {
        let rows = vec![teaching("12104", "200", 1, "Übung", 1, "11:30", "13:00").group("1-Gruppe"), teaching("12104", "200", 2, "Übung", 2, "09:15", "10:45").group("2-Gruppe"), teaching("12104", "200", 3, "Übung", 3, "11:30", "13:00").group("3-Gruppe")];
        let t = table(&rows, &["12104"], &[], &Selection { chosen_rows: [rows[0].key()].into_iter().collect(), ..Selection::default() });
        let card = card_of(&t, rows[0].key(), None);
        assert!(matches!(&card.group, Some((said, Some(CardAct::Untick(_)))) if said == "Angehakt · 1 von 3 Übungsgruppen"), "{:?}", card.group);
        assert!(card.big.is_none());
        assert_eq!(names(&card), ["hide-date", "hide-termin", "hide-event"]);
        assert!(matches!(card.hide[1].act, CardAct::Drop { .. }) && card.hide[1].sub == "die anderen 2 stehen wieder zur Wahl");
        // Another group beside it: faint, why, „Nur diese Gruppe" and „Auch diese", nothing to hide.
        let other = card_of(&t, rows[1].key(), None);
        assert_eq!(other.off.as_deref(), Some("Andere Gruppe gewählt · Mo 11:30"));
        assert!(other.big.as_ref().is_some_and(|big| big.name == "tick-only" && big.label == "Nur diese Gruppe"));
        assert!(other.also.as_ref().is_some_and(|also| also.name == "tick-also" && matches!(also.act, CardAct::Tick { .. })));
        assert!(other.hide.is_empty() && !other.change);
    }

    #[test]
    fn a_termin_the_plan_leaves_out_says_why_and_what_brings_it_back() {
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45"), teaching("12104", "100", 2, "Vorlesung", 5, "09:15", "10:45")];
        let key = rows[0].key();
        let by = |selection: Selection| card_of(&table(&rows, &["12104"], &[], &selection), key, None);
        let row = by(Selection { hidden_rows: [key].into_iter().collect(), ..Selection::default() });
        assert_eq!(row.off.as_deref(), Some("Ausgeblendet"));
        assert!(row.big.as_ref().is_some_and(|big| big.name == "show-termin" && big.label == "Einblenden" && big.said.is_none()));
        assert!(row.hide.is_empty() && !row.change);
        let event = by(Selection { hidden_events: [100].into_iter().collect(), ..Selection::default() });
        assert_eq!(event.off.as_deref(), Some("Vorlesung ausgeblendet"));
        assert!(event.big.as_ref().is_some_and(|big| big.name == "show-event" && big.label == "Vorlesung einblenden" && matches!(big.act, CardAct::Show { event: Some(100), .. })));
        let kind = by(Selection { hidden_kinds: folia_calendar::kind::KindSet::default().with(EventKind::Lecture), ..Selection::default() });
        assert_eq!(kind.off.as_deref(), Some("„Vorlesung“ ausgeblendet"));
        assert!(kind.big.as_ref().is_some_and(|big| big.name == "show-kind" && big.label == "„Vorlesung“ wieder einblenden" && matches!(big.act, CardAct::Show { kind: Some(EventKind::Lecture), .. })));
        assert!(kind.hide.is_empty() && !kind.change);
    }

    #[test]
    fn a_date_qis_cancels_offers_a_makeup_on_its_line() {
        let mut lecture = teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45");
        lecture.0.cancelled_dates = Some("17.11.2026: Krankheit".into());
        let rows = vec![lecture];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let card = card_of(&t, rows[0].key(), Some("2026-11-17"));
        assert!(card.of_date.iter().any(|line| line.makeup && line.hint.text == "Krankheit"), "{:?}", card.of_date);
        assert_eq!(names(&card), ["hide-event"]);
    }

    #[test]
    fn a_termin_of_ones_own_says_its_kind_and_skips_a_day() {
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45")];
        let own = OwnTermin { number: 1, rhythm: OwnRhythm::Week, weekday: 5, dates: Vec::new(), from: 13 * 60, to: 15 * 60, period: None, module: Some("12104".into()), name: String::new(), place: "Hörsaal".into(), skip: Vec::new(), kind: Some(EventKind::Lecture) };
        let key = own.key();
        let t = table(&rows, &["12104"], &[], &Selection { own: vec![own], ..Selection::default() });
        let shown = shown_of(&t, &Against::Itself, key, Some(d("2026-11-20")), Some((d("2026-11-10"), 600)), &modules(), (TerminMode::Adjust, Some(true)), &i18n::DE).unwrap();
        assert_eq!((shown.head.chip.as_str(), shown.head.kind.as_str()), ("EvS", "Vorlesung"));
        let card = shown.card.unwrap();
        assert_eq!(names(&card), ["hide-date"]);
        assert!(matches!(card.hide[0].act, CardAct::OwnDate { number: 1, hide: true, .. }));
        assert_eq!(card.hide[0].label, "Nur am Fr 20.11. ausblenden");
        // Its name is made of kind and module, not written by the student: no quotes.
        assert_eq!(card.hide[0].said, "VL EvS am Fr 20.11. ausgeblendet");
        assert!(card.own_show.is_none());
        assert!(card.all.is_none() && card.group.is_none() && card.big.is_none() && !card.change);
        assert!(card.series.as_deref().is_some_and(|series| series.starts_with("Jede Woche · ")), "{:?}", card.series);
    }

    #[test]
    fn who_is_only_who_the_termin_names() {
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45")];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let shown = shown_of(&t, &Against::Itself, rows[0].key(), None, None, &modules(), (TerminMode::Adjust, Some(true)), &i18n::DE).unwrap();
        // The module has a responsible, whom the overview names; the card does not.
        assert!(!shown.people.is_empty());
        assert!(shown.card.unwrap().who.is_empty());
    }

    #[test]
    fn the_cards_acts_change_the_plan_as_the_slots_do() {
        let sem = SemesterKey::parse("2026W").unwrap();
        let (a, b) = (RowKey::parse("200-aaaaa").unwrap(), RowKey::parse("200-bbbbb").unwrap());
        let lines = |doc: &PlanDoc, tag: &str| doc.stored().lines().filter(|line| line.starts_with(&format!("{tag}\t"))).count();
        let mut doc = PlanDoc::default();
        CardAct::Tick { event: 200, row: a, rows: vec![a] }.apply(&mut doc, sem);
        assert_eq!(lines(&doc, "c"), 1);
        CardAct::Drop { ticks: vec![a], rows: vec![a] }.apply(&mut doc, sem);
        assert_eq!((lines(&doc, "c"), lines(&doc, "r")), (0, 1));
        CardAct::Only { event: 200, row: b, rows: vec![b] }.apply(&mut doc, sem);
        CardAct::Tick { event: 200, row: a, rows: vec![a] }.apply(&mut doc, sem);
        assert_eq!((lines(&doc, "c"), lines(&doc, "r")), (2, 0));
        CardAct::Only { event: 200, row: b, rows: vec![b] }.apply(&mut doc, sem);
        assert_eq!(lines(&doc, "c"), 1);
        CardAct::Untick(vec![b]).apply(&mut doc, sem);
        assert_eq!(lines(&doc, "c"), 0);
        doc.set_kind(sem, EventKind::Lecture, true);
        assert_eq!(lines(&doc, "k"), 1);
        CardAct::Show { event: None, kind: Some(EventKind::Lecture), rows: Vec::new() }.apply(&mut doc, sem);
        assert_eq!(lines(&doc, "k"), 0);
        // An option among the module's events taken instead of the others, then left and hidden.
        let (x, y) = (RowKey::parse("61-aaaaa").unwrap(), RowKey::parse("62-bbbbb").unwrap());
        doc.choose(sem, 62, Some(y));
        doc.set_event(sem, 61, true);
        CardAct::Take { others: vec![61, 62], event: 61, row: x, rows: vec![x] }.apply(&mut doc, sem);
        let chosen = |doc: &PlanDoc| doc.hidden.get(&sem).map(|hides| hides.chosen.iter().copied().collect::<Vec<_>>()).unwrap_or_default();
        assert_eq!((chosen(&doc), lines(&doc, "e")), (vec![x], 0));
        CardAct::Take { others: Vec::new(), event: 62, row: y, rows: vec![y] }.apply(&mut doc, sem);
        assert_eq!(chosen(&doc), [x, y]);
        CardAct::Leave(61).apply(&mut doc, sem);
        assert_eq!((chosen(&doc), lines(&doc, "e")), (vec![y], 1));
        // What the card did is taken back alone: a tick made meanwhile stays.
        let before = doc.clone();
        let undo = PlanUndo::made(before.clone(), |doc| CardAct::Rows(vec![a]).apply(doc, sem));
        let mut now = undo.after.clone().unwrap();
        now.tick(sem, b, true);
        undo.restore(&mut now);
        assert_eq!((lines(&now, "r"), chosen(&now).contains(&b)), (lines(&before, "r"), true));
    }

    #[test]
    fn a_termin_of_one_date_and_a_group_beside_ticked_ones() {
        // One date: „Diesen Termin", not „Alle 1"; no „Vergleichen ›" beside the required Termin.
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45"), teaching("12104", "100", 2, "Übung", 3, "09:15", "10:45").rhythm("single").range("2026-11-04", "2026-11-04")];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let card = card_of(&t, rows[1].key(), None);
        assert!(card.hide.iter().any(|choice| choice.name == "hide-termin" && choice.label == "Diesen Termin"), "{:?}", card.hide);
        assert!(card.compare.is_none());
        // Two groups ticked: taking one away leaves the other ticked, nothing stands to choose again.
        let groups = vec![teaching("12104", "200", 1, "Übung", 1, "11:30", "13:00").group("1-Gruppe"), teaching("12104", "200", 2, "Übung", 2, "09:15", "10:45").group("2-Gruppe"), teaching("12104", "200", 3, "Übung", 3, "11:30", "13:00").group("3-Gruppe")];
        let t = table(&groups, &["12104"], &[], &Selection { chosen_rows: [groups[0].key(), groups[1].key()].into_iter().collect(), ..Selection::default() });
        let card = card_of(&t, groups[0].key(), None);
        assert_eq!(card.hide[1].sub, "die andere angehakte bleibt");
        assert!(card.compare.is_some());
    }

    #[test]
    fn english_sentences_take_the_kinds_labels() {
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45"), teaching("12104", "100", 2, "Vorlesung", 5, "09:15", "10:45")];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let card = card_in(&t, rows[0].key(), Some("2026-11-10"), &i18n::EN);
        assert_eq!(card.hide[2].label, "The whole lecture");
        assert_eq!(card.hide[2].said, "Lecture EvS hidden");
        let hidden = table(&rows, &["12104"], &[], &Selection { hidden_events: [100].into_iter().collect(), ..Selection::default() });
        let card = card_in(&hidden, rows[0].key(), None, &i18n::EN);
        assert_eq!(card.off.as_deref(), Some("Lecture hidden"));
        assert_eq!(card.big.map(|big| big.label).as_deref(), Some("Show the lecture"));
        // German keeps QIS's word.
        let card = card_of(&hidden, rows[0].key(), None);
        assert_eq!(card.big.map(|big| big.label).as_deref(), Some("Vorlesung einblenden"));
    }

    #[test]
    fn an_option_among_the_modules_events_is_taken_from_its_card() {
        use folia_timetable::model::tests::{choice_row, choosing, listed_at};
        let rows = [
            teaching("12104", "60", 1, "Vorlesung", 1, "15:30", "17:00"),
            teaching("12104", "61", 1, "Übung", 2, "07:30", "09:00").option(1),
            teaching("12104", "62", 1, "Übung", 3, "13:45", "15:15").option(2),
        ];
        // One taken: the other says so and is taken instead or beside; the one taken is left as it hides.
        let choices = [choice_row("12104", "group", 1, 1, "61"), choice_row("12104", "group", 1, 2, "62")];
        let taken = choosing(&rows, &["12104"], &choices, &[], &Selection { chosen_rows: [rows[2].key()].into(), ..Selection::default() });
        let other = card_of(&taken, rows[1].key(), None);
        assert_eq!(other.off.as_deref(), Some(i18n::DE.termin.other_option));
        assert!(other.big.as_ref().is_some_and(|big| big.name == "take-only" && matches!(&big.act, CardAct::Take { others, event: 61, .. } if others == &[61, 62])));
        assert!(other.also.as_ref().is_some_and(|also| also.name == "take-also" && matches!(&also.act, CardAct::Take { others, .. } if others.is_empty())));
        let chosen = card_of(&taken, rows[2].key(), None);
        assert!(chosen.hide.iter().any(|choice| choice.act == CardAct::Leave(62)), "{:?}", chosen.hide);
        // Another program's option: „Diese nehmen".
        let program = [choice_row("12104", "program", 1, 1, "61"), choice_row("12104", "program", 1, 2, "62")];
        let theirs = choosing(&rows, &["12104"], &program, &[listed_at("12104", 2)], &Selection::default());
        let card = card_of(&theirs, rows[1].key(), None);
        assert_eq!(card.off.as_deref(), Some(i18n::DE.termin.other_programs));
        assert!(card.big.as_ref().is_some_and(|big| big.name == "take" && matches!(&big.act, CardAct::Take { event: 61, .. })));
        assert!(card.hide.is_empty() && !card.change);
    }

    #[test]
    fn to_look_up_there_is_no_card() {
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45")];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let look = shown_of(&t, &Against::Nothing, rows[0].key(), None, None, &modules(), (TerminMode::LookUp, Some(false)), &i18n::DE).unwrap();
        assert!(look.card.is_none());
    }

    #[test]
    fn the_card_stands_beside_its_slots_column() {
        let room = Room { left: 64.0, width: 1440.0, height: 900.0 };
        let slot = |left: f64, right: f64, top: f64, bottom: f64| Some(Anchor { left, top, right, bottom });
        // A card of 330 px that can grow to 450.
        let size = Size { rest: 330.0, body: 200.0, tallest: 320.0 };
        let place = |at, room| card_place(at, 400.0, room, size);
        let top = |left: f64, top: f64| Placed { left, edge: Edge::Top(top) };
        assert_eq!(place(slot(400.0, 540.0, 300.0, 380.0), room), top(550.0, 300.0));
        assert_eq!(place(slot(1100.0, 1240.0, 300.0, 380.0), room), top(690.0, 300.0));
        assert_eq!(place(slot(700.0, 840.0, 300.0, 380.0), room), top(290.0, 300.0));
        assert_eq!(place(slot(380.0, 500.0, 300.0, 380.0), Room { left: 64.0, width: 920.0, height: 800.0 }), top(504.0, 300.0));
        assert_eq!(place(slot(420.0, 560.0, 100.0, 190.0), Room { left: 64.0, width: 901.0, height: 800.0 }), top(80.0, 200.0));
        assert_eq!(place(None, room), top(552.0, 108.0));
        for left in [0.0, 70.0, 300.0, 1000.0] {
            assert!(place(slot(left, left + 100.0, 300.0, 380.0), room).left >= 80.0);
        }
        // Lower, where it could not grow under the slot's top: its top still at the slot's, its
        // bottom where it ends as it rests, growing upward.
        assert_eq!(place(slot(400.0, 540.0, 500.0, 580.0), room), Placed { left: 550.0, edge: Edge::Bottom(900.0 - 830.0) });
        // So low that even the resting card does not fit: its bottom 16 px over the window's edge.
        assert_eq!(place(slot(400.0, 540.0, 700.0, 780.0), room), Placed { left: 550.0, edge: Edge::Bottom(16.0) });
        // A card that never grows keeps its top at the slot's as long as it fits.
        let still = Size { rest: 330.0, body: 200.0, tallest: 0.0 };
        assert_eq!(card_place(slot(400.0, 540.0, 540.0, 620.0), 400.0, room, still), top(550.0, 540.0));
    }

    #[test]
    fn a_card_reserves_what_its_bodies_can_grow_to() {
        let rows = vec![teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45"), teaching("12104", "100", 2, "Vorlesung", 5, "09:15", "10:45")];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let card = card_of(&t, rows[0].key(), Some("2026-11-10"));
        assert_eq!(tallest_body(&card, false, true), BODY_CHANGE);
        // A Termin the plan leaves out: no pencil, no eye; the note's field alone.
        let hidden = table(&rows, &["12104"], &[], &Selection { hidden_events: [100].into_iter().collect(), ..Selection::default() });
        let card = card_of(&hidden, rows[0].key(), None);
        assert_eq!(tallest_body(&card, false, true), BODY_NOTE);
        assert_eq!(tallest_body(&card, true, false), 0.0);
        assert_eq!(Size { rest: 300.0, body: 200.0, tallest: 100.0 }.most(), 300.0);
        assert_eq!(Size { rest: 300.0, body: 200.0, tallest: 345.0 }.most(), 445.0);
    }
}
