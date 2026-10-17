//! What the document of every page links and runs: where the host serves the files of
//! `folia/assets`, the colours of the browser's own chrome, and the script before the first paint.

/// The release of Folia (2026-09-21: Folia and Radix are both alpha-0.2.0; 2026-09-22: Folia
/// alpha-0.2.1 with the phone's filter sheet; 2026-09-23: Folia alpha-0.2.2, Radix alpha-0.3.0;
/// 2026-09-27, after the public release: Folia 1.0.1, Radix 0.5.0, no stage in front any more;
/// 2026-09-29: Folia 1.0.2, the same day 1.0.3 with the wood behind every page; until then the
/// owner named each). Since 2026-10-15 by SemVer, raised with every change (owner: „immer wenn wir
/// changes machen update die version number based on sem ver"; CLAUDE.md, „Versions"): 1.1.0 with
/// Betula's icon „Tinte" and the start of the installed app; 1.2.0 with nothing said of the data in
/// the top bar any more (neither its loading nor why the app does not start offline); 1.3.0 with a
/// program's page on a phone starting with the program, „Zurück" at the head instead of the way to
/// the profile page, and no actions on a program's page; 1.3.1 with its head on a phone without
/// loose ends; 1.4.0 with the program an area has chosen kept for the session alone and „Mein
/// Studiengang" first in every program picker; 1.5.0 with every „Zurück" at the head of a phone's
/// page; 1.6.0 with the dialogs in three sizes, „Schließen Esc" and the buttons on the right, the
/// app's select in place of the browser's and the Regelstudienplan's heads in rows; 1.7.0 with a
/// module's sheet on a phone that comes up from below in one move, goes on as the finger let it
/// go, on a spring, and has no „Vollbild"; 1.8.0 with the tabs of the navigation a digit away (Alt,
/// or Ctrl and Alt, and 1–5); 1.9.0 with „Dein Studium" as a card in „Studiengang wechseln" and on
/// the first visit, the Fachsemester counted with − and +, the study direction a row of it, nothing
/// moving while it is used; 1.9.1 with „Keine zur Wahl" whole in a phone's row; 1.10.0 with the
/// navigation's keys shown at once and in full, „Alt + 1"; 1.11.0 with the study direction's row of
/// the card only where the program has study directions, unfolding as such a program is picked;
/// 1.12.0 with a module's Termine folded under its week, a slot showing its line, the facts two side
/// by side and „Studiengänge" last, five of them; 1.13.0 with a phone's pickers and fields 8 px off
/// the edges, a dialog or a sheet the part of the screen its keyboard leaves, and the note of
/// „Studiengang wechseln" a line at the foot of its body; 1.14.0 with every text field up at the
/// top of what scrolls it while the keyboard is there, not only a picker; 1.15.0 with all of it
/// moving rather than jumping: the dialog or the sheet gliding over the keyboard and back, what is
/// typed in scrolling up smoothly, an open picker's button drawing out and its list unrolling. The
/// version of the app's crates, the same as the server's; Radix's is in the snapshot
/// (`Meta::radix_version`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the host serves the files of `folia/assets`.
pub const STYLESHEET: &str = "/assets/app.css";
pub const FAVICON: &str = "/assets/favicon.svg";
/// The icon of Betula everywhere (owner, 2026-10-15: „Tinte", the birch leaf in paper on a square
/// of ink with the bars of birch bark cut out of it, turned over in the dark;
/// `design/logo/app-icon.mjs`, and `design/logo/render-icons.mjs` makes the pictures): the tab's
/// SVG, turned over where the browser is dark, and the pictures for what cannot read it: the
/// classic `/favicon.ico`, the icon of iOS (home screen, link previews of Messages) and the icons of
/// the web app manifest, plain ones for desktops, maskable ones for Android's launchers and the
/// splash screen of the installed app, and the monochrome one that Android's themed icons tint in
/// the colours of the wallpaper. The leaf stands whole in whatever shape a launcher cuts.
pub const FAVICON_ICO: &str = "/favicon.ico";
pub const TOUCH_ICON: &str = "/apple-touch-icon.png";
/// Also the picture Google Search shows beside the site's results (owner, 2026-10-01: the icon of
/// the app there). Google takes one per host from the start page's `icon` and `apple-touch-icon`
/// links, reads no SVG and asks for a square larger than 48 px; how it chooses among several it
/// does not document (as far as can be seen, the largest it reads). Linked as an `icon` of 192 px,
/// this one is the largest of either kind. Chromium and Firefox take the SVG for the tab whatever
/// else is linked (Chromium does not even fetch this one).
pub const ICON_192: &str = "/assets/icon-192.png";
pub const ICON_512: &str = "/assets/icon-512.png";
/// The icon of Android's launchers, in both manifests: Chrome asks for 83 dp there and takes the
/// icon of that size or the next larger one, so 334 px serve every phone up to a density of 4.02.
pub const ICON_MASKABLE: &str = "/assets/icon-maskable-334.png";
/// The icon of the splash screen of the installed app, which Chrome draws at 128 dp: from a density
/// of 2.61 on it asks for more than 334 px and takes this one, never the launcher's. The dark
/// manifest names the turned-over one in its place, so that only the splash screen turns over in
/// the dark: an icon of the launcher that changed with the theme would make Chrome ask the visitor
/// about the app's new icon each time.
pub const ICON_SPLASH: &str = "/assets/icon-maskable-512.png";
pub const ICON_SPLASH_DARK: &str = "/assets/icon-maskable-dark-512.png";
pub const ICON_MONOCHROME: &str = "/assets/icon-monochrome-512.png";
/// Name, colours and icons of the site for a home screen or an installed window.
pub const MANIFEST: &str = "/manifest.webmanifest";
/// The same in the colours of the dark theme, which a page in the dark names instead (the head
/// script, the theme switch). Chrome on Android (158 as of 2026-10) reads one `theme_color` and
/// one `background_color` from a manifest and no dark ones (`color_scheme_dark` of the draft is not
/// parsed): the splash screen of the installed app is the background, the strip under its gesture
/// bar the theme colour (and without one the window's background, the same colour), all the time
/// the app is open, whatever the page says. So the installed app takes the colours of the
/// manifest the page named, and Chrome brings them up to date from the manifest a page names when
/// they differ (the same `id`, looked at once a day at most): a phone in the dark gets a dark
/// splash and a dark strip.
pub const MANIFEST_DARK: &str = "/manifest-dark.webmanifest";
/// The page background of the light and the dark theme (`--bg`), for the browser's own chrome.
pub const THEME_LIGHT: &str = "#f1f2f4";
pub const THEME_DARK: &str = "#0a0c11";
pub const FONT: &str = "/assets/inter-latin.woff2";
/// The picture of link previews (1200 × 630, made from `design/og/og.html`).
pub const OG_IMAGE: &str = "/assets/og.png";
/// The screenshots of the start page's carousel: `<SHOTS>/<name>[-phone][-dark].webp`.
pub const SHOTS: &str = "/assets/shots";
pub const ENHANCE_SCRIPT: &str = "/assets/enhance.js";
/// Loads the local database and the browser app, which then takes the page over.
pub const BOOT_SCRIPT: &str = "/assets/boot.js";
/// The service worker: keeps the shell of the app for a start without a network (`boot.js`
/// registers it; the web server writes its build into it).
pub const SERVICE_WORKER: &str = "/sw.js";

/// Runs before the first paint: marks the document as scripted, names the season the birch is
/// drawn in (`data-season`: March–May spring, June–August summer, September–November autumn,
/// December–February winter; the server's page is the same all year, R9), and applies what this browser
/// remembers (theme, widths of the filter panel and the module preview), so nothing flashes or jumps; the
/// colour of the browser's own chrome (`theme-color`, `THEME_DARK`) follows the theme, and so does
/// the manifest the page names (`MANIFEST_DARK`), whose colours an installed app takes. Such personal
/// view settings live in localStorage, never in the URL and never in server HTML (R9). A browser
/// that keeps „Mein Studiengang" in the active profile (`folia_stores::profiles`: the first keeps
/// it under the plain key, another under its number) marks the document with `mine`: the program
/// overview's line about it is the app's, and the page keeps its room from the first paint, so the
/// list does not move when the app takes over (R15). On an iPhone or iPad (`navigator.standalone` exists there only)
/// it names the launch screens of this screen, upright and, on a tablet, turned, light and dark
/// (`launch`): iOS takes them when the app is added to the home screen, from the page as it is
/// then. Written here and not as sixty `<link>`s into every page, which would cost every visitor
/// almost a kilobyte for what only a home screen of iOS reads. And where the installed app starts on
/// a phone or a tablet (`display-mode: standalone`, or `navigator.standalone` on iOS, and a coarse
/// pointer), once a session (`betula.launched` in sessionStorage: a link or a reload in the app is
/// no start), it lays the start of the app over the page before its first paint (`.launch`,
/// app.css; owner, 2026-10-15: concept A of `design/launch/konzepte.html`): the picture of the
/// system's splash screen, out of which enhance.js lets the icon fly to the page's own. It goes
/// beside the body, not into it, where the app takes the page over. iOS's picture stands on the
/// whole screen: the screen's geometry places it (`--mid`, the screen's middle, and `--name`, the
/// baseline of the name, as `launch` draws them, less the status bar above the window). Not on the
/// login page of closed testing, which names no manifest and runs no enhance.js. Public for that
/// one document the server writes without the app.
pub const HEAD_SCRIPT: &str = "var d=document.documentElement;d.classList.add('js');var n=new Date().getMonth();d.dataset.season=n<2||n>10?'winter':n<5?'spring':n<8?'summer':'autumn';try{var t=localStorage.getItem('betula.theme');if(t==='dark'||t==='light')d.dataset.theme=t;else t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';if(t==='dark'){var m=document.querySelector('meta[name=theme-color]');if(m)m.content='#0a0c11';m=document.querySelector('link[rel=manifest]');if(m)m.href=m.getAttribute('href').replace('/manifest.webmanifest','/manifest-dark.webmanifest')}var w=parseInt(localStorage.getItem('betula.preview.width'),10);if(w>=360&&w<=2400)d.style.setProperty('--preview-w',w+'px');var f=parseInt(localStorage.getItem('betula.filters.width'),10);if(f>=232&&f<=440)d.style.setProperty('--w-filters',f+'px');var p=/^active\\t([1-9][0-9]{0,2})$/m.exec(localStorage.getItem('betula.profiles.v1')||''),k=p&&p[1]!=='1'?'betula.p'+p[1]+'.myprogram.v1':'betula.myprogram.v1';if(/^program\\t[0-9A-Za-z]/m.test(localStorage.getItem(k)||''))d.classList.add('mine')}catch(e){}if('standalone'in navigator)(function(){var s=screen,r=Math.round(devicePixelRatio),a=Math.min(s.width,s.height)*r,b=Math.max(s.width,s.height)*r;(a<1400?[[a,b,'portrait']]:[[a,b,'portrait'],[b,a,'landscape']]).forEach(function(o){['light','dark'].forEach(function(c){var l=document.createElement('link');l.rel='apple-touch-startup-image';l.media='(orientation: '+o[2]+') and (prefers-color-scheme: '+c+')';l.href='/assets/launch/'+o[0]+'x'+o[1]+(c==='dark'?'-dark':'')+'.png';document.head.appendChild(l)})})})();(function(){try{if((navigator.standalone||matchMedia('(display-mode: standalone)').matches)&&matchMedia('(pointer: coarse)').matches&&!sessionStorage.getItem('betula.launched')){sessionStorage.setItem('betula.launched','1');if(document.querySelector('link[rel=manifest]')){var o=document.createElement('div'),i=navigator.standalone;o.className=i?'launch ios':'launch';o.setAttribute('aria-hidden','true');o.innerHTML='<div class=\"launch-icon\"><b></b><i></i></div>'+(i?'<div class=\"launch-name\"><i></i></div>':'');if(i){var s=screen,h=innerHeight,H=innerWidth>h?Math.min(s.width,s.height):Math.max(s.width,s.height),f=Math.max(H-h,0);o.style.cssText='--mid:'+(H/2-f)+'px;--name:'+(H-Math.max(.075*H,44)-f)+'px'}d.appendChild(o)}}}catch(e){}})()";

/// The opt-in to the fade between pages, in the head of every document the server writes (this
/// shell and the login page of closed testing). Not in app.css: Chromium decides whether a new
/// page takes part in the transition when it shows it for the first time, from the style sheets it
/// has applied by then, and the stylesheet, revalidated on every page load, often arrives after the
/// parser has reached `<body>`. The page then came without the fade and with "Transition was
/// aborted because of invalid state. ViewTransition opt-in disabled" in the console. Written
/// inline, the rule is there before the body is. The duration stays in app.css.
pub const VIEW_TRANSITION_STYLE: &str = "@view-transition{navigation:auto}";

#[cfg(test)]
mod tests {
    use folia_stores::profiles::{key_of, KEYS, STORAGE_KEY};

    /// The head script reads „Mein Studiengang" where the active profile keeps it, as the store
    /// names the key.
    #[test]
    fn the_head_script_reads_the_active_profiles_program() {
        let mine = KEYS[3];
        assert_eq!((mine, key_of(mine, 2).as_str()), ("betula.myprogram.v1", "betula.p2.myprogram.v1"));
        assert!(super::HEAD_SCRIPT.contains(&format!("localStorage.getItem('{STORAGE_KEY}')")), "{}", super::HEAD_SCRIPT);
        assert!(super::HEAD_SCRIPT.contains("'betula.p'+p[1]+'.myprogram.v1':'betula.myprogram.v1'"), "{}", super::HEAD_SCRIPT);
    }

    /// In the dark the head script turns the browser's chrome dark and names the dark manifest in
    /// place of the light one, a language's as well (`/en/manifest.webmanifest`).
    #[test]
    fn the_head_script_names_the_dark_manifest_in_the_dark() {
        use super::{HEAD_SCRIPT, MANIFEST, MANIFEST_DARK, THEME_DARK};
        assert!(HEAD_SCRIPT.contains(&format!("m.content='{THEME_DARK}'")), "{HEAD_SCRIPT}");
        assert!(HEAD_SCRIPT.contains(&format!("m.href=m.getAttribute('href').replace('{MANIFEST}','{MANIFEST_DARK}')")), "{HEAD_SCRIPT}");
    }
}
