//! Reproduction of docs/frontend-rewrite.md §3A: pages swapped by a keyed <For>, each page
//! subscribing to an App-level signal that changes in the same step as the page id, and
//! reading page-owned values in its effects.
use leptos::prelude::*;

#[component]
fn Page(id: u32, tab: Signal<u32>, shared: Signal<u32>) -> impl IntoView {
    // Page-owned state, like `selected_area_filter` in program_detail.rs.
    let local = RwSignal::new(0u32);
    Effect::new(move |_| {
        let t = tab.get();
        let l = local.get();
        let s = shared.get();
        web_sys::console::log_1(&format!("page {id}: effect tab={t} local={l} shared={s}").into());
    });
    let label = Memo::new(move |_| format!("page {id} tab {} local {}", tab.get(), local.get()));
    let detail = StoredValue::new(Some(id));
    view! {
        <section>
            <h1 id="label">{move || label.get()}</h1>
            <button id="local" on:click=move |_| local.update(|v| *v += 1)>"local"</button>
            // As in program_detail.rs: a reactive block whose children read the
            // page-owned wrapper signal in attribute closures.
            {move || match detail.get_value() {
                None => view! { <p>"missing"</p> }.into_any(),
                Some(_) => view! {
                    <div>
                        <button class=move || format!("tab {}", if tab.get() == 0 { "active" } else { "" })>"Plan"</button>
                        <button class=move || format!("tab {}", if tab.get() == 1 { "active" } else { "" })>"Bereiche"</button>
                        <Show when=move || local.get() % 2 == 0 fallback=|| view! { <i>"odd"</i> }>
                            <span>{move || shared.get()}</span>
                        </Show>
                    </div>
                }.into_any(),
            }}
        </section>
    }
}

#[component]
fn App() -> impl IntoView {
    let (page, set_page) = signal(1u32);
    let (tab, set_tab) = signal(0u32);
    let (shared, set_shared) = signal(0u32);
    let visits = RwSignal::new(0u32);

    let go = move |_| {
        // Same step: the tab and the page id change together.
        set_tab.update(|t| *t = (*t + 1) % 3);
        set_shared.update(|s| *s += 1);
        set_page.update(|p| *p += 1);
        visits.update(|v| *v += 1);
    };

    // The other order: the page id first, then the signals the outgoing page subscribes to.
    let go2 = move |_| {
        set_page.update(|p| *p += 1);
        set_tab.update(|t| *t = (*t + 1) % 3);
        set_shared.update(|s| *s += 1);
        visits.update(|v| *v += 1);
    };

    view! {
        <button id="go" on:click=go>"next page"</button>
        <button id="go2" on:click=go2>"next page, id first"</button>
        <p id="visits">{move || visits.get()}</p>
        <For
            each=move || vec![page.get()]
            key=|p| *p
            children=move |p| view! {
                // `.into()` inside the <For>: the wrapper is owned by the page.
                <Page id=p tab=tab.into() shared=shared.into() />
            }
        />
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
