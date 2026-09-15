use leptos::prelude::*;

/// Responsive App Window Layout with Sidebar (docked/drawer), Backdrop, Sticky Header Navbar, and Main Content Focus
#[component]
pub fn AppLayout<S, N, SView, NView>(
    sidebar_open: Signal<bool>,
    on_close_sidebar: Callback<()>,
    sidebar: S,
    navbar: N,
    children: Children,
) -> impl IntoView
where
    S: Fn() -> SView + Send + Sync + 'static,
    N: Fn() -> NView + Send + Sync + 'static,
    SView: IntoView + 'static,
    NView: IntoView + 'static,
{
    view! {
        <div class=move || if sidebar_open.get() { "app-layout sidebar-open" } else { "app-layout" }>
            <aside class=move || if sidebar_open.get() { "sidebar open" } else { "sidebar" } id="sidebar">
                {sidebar()}
            </aside>

            <div
                class=move || if sidebar_open.get() { "sidebar-backdrop active" } else { "sidebar-backdrop" }
                id="sidebar-backdrop"
                on:click=move |_| on_close_sidebar.run(())
            ></div>

            <div class="main-wrapper">
                {navbar()}
                <main class="main-content">
                    {children()}
                </main>
            </div>
        </div>
    }
}
