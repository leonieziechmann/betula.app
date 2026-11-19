//! „Bereiche", a view of „Mein Studium" (`/study?view=progress`): what each area of the progress
//! holds of what it asks, and an area of it with what counts there (`area=<n>`, `area=none` what
//! counts towards none). On a phone the page a tap on the overview's bar leads to, an area a page
//! of its own behind it, as the Wahlpflicht has its choices (`choose.rs`); a module of an area
//! opens as the sheet over its page, and closing it leaves the page as it was (owner, 2026-11-17,
//! of the areas as a sheet: „wenn man auf ein modul klickt von der ansicht, weil das ist modal zu
//! modal und das sieht richtig komisch aus. Also entweder stacken sich modals oder das wird zur
//! eigenen seite"). Until then the areas were a sheet over the overview and an area the same
//! sheet, whose module closed it and opened a sheet of its own in its place.
//!
//! The way a level up is the way back at the head of a phone's page (`chrome::TopBack`, the box
//! left of the search): from an area to the list, from the list to the overview, through the
//! history where that is the step before; Back on Android goes the same way. On a desktop the
//! overview's cards open an area as a dialog (`overview::AreaDialog`); an address of this view
//! shows its page in a panel there, an area's with a way to the list at its head.

use folia_routes::url::{LocalView, StudyUrl};
use folia_shell::chrome::{Back, TopBack};
use leptos::prelude::*;

use folia_design::ui::Icon;

use super::choose::bar as stand_bar;
use super::overview::{figure_of, tally_line, tally_strip, AreaBody, AreaInfo, Info};
use super::StudyCtx;
use crate::i18n;

/// The view: the list of the areas, or the area of the address.
#[component]
pub(super) fn Areas(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let v = &t.study_views;
    let info = Memo::new(move |_| ctx.with_ready(|ready| Info::of(ready, t)));
    // The area of the address, where the study has it; else the list.
    let picked = Memo::new(move |_| {
        let area = ctx.url.with(|url| url.area)?;
        ctx.with_ready(|ready| AreaInfo::of(ready, area, t)).flatten()
    });
    // The way back at the head of a phone's page: from an area to the list, from the list to the
    // overview, through the history where that is the step before.
    TopBack::follow(move || {
        if !ctx.phone.get() {
            return None;
        }
        Some(match picked.with(Option::is_some) {
            true => Back { href: ctx.url.with(|url| url.with_area(None).with_module(None, false).path()), label: s.areas, history: ctx.area_back.get() },
            false => Back { href: StudyUrl::default().path(), label: v.to_overview, history: ctx.view_back.get() },
        })
    });
    move || {
        let phone = ctx.phone.get();
        match picked.get() {
            Some(area) => Some(view! { <AreaPage ctx area phone/> }.into_any()),
            None => info.get().map(|info| view! { <ListPage ctx info phone/> }.into_any()),
        }
    }
}

/// The areas: what is passed of what the plan asks, the strip of the four marks with the study's
/// credits, and a card per area — its name and what it holds of what it asks, its bar in its tone,
/// its marks with their credits —, each the way to the area's page.
#[component]
fn ListPage(ctx: StudyCtx, info: Info, phone: bool) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let cards = info
        .cards
        .into_iter()
        .map(|card| {
            let area = card.area;
            let href = t.path(&ctx.url.with_untracked(|url| url.with_area(Some(area)).with_module(None, false).path()));
            view! {
                <li>
                    <a href=href style=format!("--c: {}", card.tone) on:click=move |_| ctx.area_back.set(true)>
                        <span class="st-area-t"><span class="st-dot"></span><b>{card.name}</b>{figure_of(&card.stand, t)}<Icon name="chevron-right"/></span>
                        {stand_bar(&card.stand, None, "", t)}
                        {tally_line(&card.tally, card.after, t)}
                    </a>
                </li>
            }
        })
        .collect_view();
    let body = view! {
        {tally_strip(&info.tally, t)}
        <ul class="st-area-list" data-stop="y">{cards}</ul>
    };
    if phone {
        view! {
            <div class="st-phone-page st-areas-phone">
                <header class="st-vhead">
                    <h1 id="st-areas-title">{s.areas}</h1>
                    <p>{info.headline}</p>
                </header>
                <section class="panel st-areas-page" aria-labelledby="st-areas-title">{body}</section>
            </div>
        }
        .into_any()
    } else {
        view! {
            <section class="panel st-areas-page" aria-labelledby="st-areas-title">
                <header class="st-view-head">
                    <div>
                        <h2 id="st-areas-title">{s.areas}</h2>
                        <p>{info.headline}</p>
                    </div>
                </header>
                <div class="st-areas-in">{body}</div>
            </section>
        }
        .into_any()
    }
}

/// An area: its name and what it holds of what it asks at the head, what counts there under it
/// (`AreaBody`). On a desktop a way to the list at its head, which a phone has at the head of the
/// screen.
#[component]
fn AreaPage(ctx: StudyCtx, area: AreaInfo, phone: bool) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let tone = area.tone;
    let (name, asks) = (area.name.clone(), area.asks.clone());
    if phone {
        view! {
            <div class="st-phone-page st-areas-phone">
                <header class="st-vhead st-area-vhead" style=format!("--c: {tone}")>
                    <h1 id="st-area-title"><span class="st-dot"></span><span>{name}</span></h1>
                    <p>{asks}</p>
                </header>
                <section class="panel st-area-page st-area-body" style=format!("--c: {tone}") aria-labelledby="st-area-title">
                    <AreaBody ctx info=area/>
                </section>
            </div>
        }
        .into_any()
    } else {
        let list = t.path(&ctx.url.with_untracked(|url| url.with_area(None).with_module(None, false).path()));
        view! {
            <section class="panel st-area-page" style=format!("--c: {tone}") aria-labelledby="st-area-title">
                <header class="st-view-head">
                    <div>
                        <h2 id="st-area-title" class="st-area-h"><span class="st-dot"></span><span>{name}</span></h2>
                        <p>{asks}</p>
                    </div>
                    <a class="st-link" href=list><Icon name="chevron-left"/>{s.areas}</a>
                </header>
                <div class="st-area-body">
                    <AreaBody ctx info=area/>
                </div>
            </section>
        }
        .into_any()
    }
}
