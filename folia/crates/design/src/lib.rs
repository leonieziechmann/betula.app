//! The design of the app: the building blocks every page uses (`ui`, `icons`, `combobox`), how
//! numbers and dates are written (`format`), the browser's own means (`nav`: storage, sizes,
//! popups) and the languages with the texts of these parts (`i18n`, and `texts!`, with which
//! every crate of the app puts its texts together). Below the shell and the features: it knows
//! no page, no data and no store (docs/folia/folia-refactor.md §7.4).

pub mod combobox;
pub mod format;
pub mod i18n;
pub mod icons;
pub mod nav;
pub mod ui;

use leptos::prelude::*;

/// The build of the server that writes the page (given by the host, server side only). The
/// document links the stylesheet and the scripts with it (`/assets/app.css?v=<build>`), and
/// `boot.js` hands the same `?v=` on to the bundle and to sql.js. A service worker of another
/// build has nothing under such an address and asks the network, so a page always gets the
/// stylesheet, the scripts and the bundle of its own build — also on the first load after a
/// deploy, which the worker of the old build still answers. Every visitor gets the same build,
/// so the server's HTML stays the same for everybody (R9).
#[derive(Clone)]
pub struct BuildId(pub std::sync::Arc<str>);

impl BuildId {
    /// The address under which a page of this build asks for `path`.
    pub fn asset(&self, path: &str) -> String {
        format!("{path}?v={}", self.0)
    }
}

/// `path` as the page being rendered links it: with the build its host gave (`BuildId`), plain
/// where no host gave one.
pub fn asset(path: &str) -> String {
    use_context::<BuildId>().map_or_else(|| path.to_string(), |build| build.asset(path))
}
