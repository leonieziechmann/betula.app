//! The switch between the languages of the site (owner, 2026-09-27): a button per other language,
//! to the page the visitor is on with what its address says (filters, the module beside it). On
//! the desktop it stands in the rail above the switch between light and dark; on a phone, which
//! has no rail, at the end of the start page's brand line: one tap away through „Start", and far
//! from the bottom bar and the lists, where a thumb could hit it by accident.
//!
//! Another language is another address and a new page load (`rel="external"`: neither the router
//! nor `pending` take it), so the app starts again in that language; what the visitor keeps in this
//! browser (Merkliste, Stundenplan, „Mein Studiengang") belongs to the site, not to a language, and
//! stays. With JavaScript the choice is remembered (`data-language`, `enhance.js`), and every later
//! visit opens in it (`language_script`); the first one opens in the language of the browser.
//!
//! The links depend on the address alone, so the server's page stays the same for everybody (R9).

use leptos::prelude::*;

use crate::i18n::{self, Locale};
use crate::tabs::location_of;
use crate::ui::Icon;

/// The switch: for every other language a button with the icon of languages and its code („EN"),
/// named in its own language.
#[component]
pub fn Languages() -> impl IntoView {
    let t = i18n::t();
    let location = i18n::use_location();
    let here = Memo::new(move |_| location_of(&location.pathname.get(), &location.search.get()));
    // The same view in another language is a view as well: no page for a crawler (`seo`).
    let rel = move || if here.with(|here| folia_routes::url::listed(here)) { "alternate external" } else { "alternate external nofollow" };
    let buttons = Locale::ALL
        .iter()
        .copied()
        .filter(move |locale| *locale != t.locale)
        .map(|locale| {
            view! {
                <a
                    class="icon-btn lang-switch"
                    href=move || here.with(|here| locale.path(here))
                    hreflang=locale.code()
                    lang=locale.code()
                    rel=rel
                    data-language=locale.code()
                    title=locale.name()
                    aria-label=locale.name()
                >
                    <Icon name="languages"/>
                    <b aria-hidden="true">{locale.code().to_uppercase()}</b>
                </a>
            }
        })
        .collect_view();
    view! { <nav class="languages" aria-label=t.common.language>{buttons}</nav> }
}

/// Runs first in the head of every page of the app (with JavaScript only): the page opens in the
/// visitor's language (owner, 2026-09-27: „in der js version automatisch auf die browser sprache
/// gewechselt … im local storage gespeichert … beim nächsten besuch wird immer auf die gespeicherte
/// sprache gewechselt"). That is the language kept in `localStorage` (`betula.language`), else the
/// first of the browser's languages the site speaks (`navigator.languages`, else the default),
/// which is then kept. A page in another language is replaced by the same address in that one,
/// before anything is drawn. The switch keeps the language it leads to before it leads there
/// (`enhance.js`), so it is never undone here.
///
/// Where nothing can be kept (storage blocked) nothing is changed: a choice made with the switch
/// could not be remembered, and the next page would take it back. Nor for crawlers and automated
/// browsers (`navigator.webdriver`): search engines have to see every language at its own address
/// (`hreflang`), not be sent to the one their browser happens to be set to.
pub fn language_script() -> String {
    let languages = Locale::ALL.iter().map(|locale| format!("['{}','{}']", locale.code(), locale.prefix())).collect::<Vec<_>>().join(",");
    format!(
        "(function(){{try{{var L=[{languages}],K='betula.language';\
if(navigator.webdriver||/bot|crawl|spider|slurp|lighthouse|preview/i.test(navigator.userAgent))return;\
var p=location.pathname,now=L[0],rest=p,i,j;\
for(i=1;i<L.length;i++)if(p===L[i][1]||p.indexOf(L[i][1]+'/')===0){{now=L[i];rest=p.slice(L[i][1].length)||'/'}}\
var kept=localStorage.getItem(K),want=null;\
for(i=0;i<L.length;i++)if(L[i][0]===kept)want=L[i];\
if(!want){{var b=navigator.languages&&navigator.languages.length?navigator.languages:[navigator.language||''];\
for(j=0;j<b.length&&!want;j++)for(i=0;i<L.length;i++)if(String(b[j]).toLowerCase().split(/[-_]/)[0]===L[i][0])want=L[i];\
want=want||L[0];localStorage.setItem(K,want[0]);if(localStorage.getItem(K)!==want[0])return}}\
if(want!==now)location.replace((want[1]&&rest==='/'?want[1]:want[1]+rest)+location.search+location.hash)\
}}catch(e){{}}}})()"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_knows_every_language() {
        let script = language_script();
        for locale in Locale::ALL {
            assert!(script.contains(&format!("['{}','{}']", locale.code(), locale.prefix())), "{script}");
        }
        // The default language comes first: a browser that speaks none of them gets it.
        assert!(script.contains(&format!("var L=[['{}','']", Locale::default().code())), "{script}");
        assert!(!script.contains('"') && !script.contains("</"), "it stands inline in the head: {script}");
    }
}
