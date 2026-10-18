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
/// typed in scrolling up smoothly, an open picker's button drawing out and its list unrolling;
/// 1.16.0 with a row swiped on a phone going from the look of what its module is to that of what
/// it will be — „Merken" to the inverted look and back, „Einplanen" to the accent, out of the plan
/// to red — and its bookmark going with the card as one; 1.16.1 with the bookmark's colours easing
/// in still; 1.17.0 with the way back at the head of „Alle Studiengänge" on a phone, up to „Mein
/// Studium"; 1.17.1 with a shared link to a module far down the catalog (`/catalog?open=11787`) no
/// longer freezing the tab as the app takes the page over; 1.18.0 with the semesters of „Einplanen"
/// besides its own, a menu under the switch — a mouse resting on it, Shift+P or P held, a right
/// click, on a phone a finger held while its border fills; 1.19.0 with that menu a building block of
/// every switch that has one, „Einplanen" among the sidebar's actions with it beside, and no
/// „Anderes Semester"; 1.20.0 with the way back and the way to the profile page at a phone's head
/// changing places as one row that glides, by transform and opacity, while the page is built;
/// 1.20.1 with a row swiped on a phone the next finger's at once, while it still says what was done
/// or glides back; 1.20.2 with the bar at a phone's bottom taken where it stands by a swipe that
/// catches its glide in the first frame, which stood still with the wrong icon in its lens and
/// moved no more; 1.21.0 with the red of a row swiped out of the plan striped at 45°, for the
/// red-green blind; 1.22.0 with the way back at the head of „Mein Studium" on a phone going with the
/// row's first move back into the overview, as it comes with its first move off; 1.23.0 with
/// „Einplanen" planning at once into the semester the session plans for and a note at the bottom
/// saying where, its ‹ › moving the module (into the past as well), „Weitere Module auch nach …" and
/// „Rückgängig", where no study is set up one dialog that sets it up and plans the module, and the
/// menu of the semesters gone; 1.24.0 with that note one bar across the page right of the rail on a
/// wide screen, one line high with the module named, on a phone its switch a row over the arrows,
/// dark in the dark theme as well, and the catalog's hint of a program's plan no longer taking its
/// look; 1.24.1 with the note's semester one control on a phone (a tap on its half, a finger
/// swiped over it), the note swiped down and away and gone as the student moves on (another page,
/// module or search, a longer scroll; × and Esc on a wide screen), on a phone under the bottom bar
/// and flat, and in the dialog for a study the search for a program the app's picker as wide as the
/// programs, a program known a row with its picker, and „Aufheben" for a semester chosen for the
/// next modules; 1.24.2 with that note a note that waits for interaction, a building block of its
/// own (`folia_design::waitnote`): no timer, it stands until the student does something else — a
/// click or a tap elsewhere, a key not its own, another page, a longer scroll; 1.24.3 with the note
/// on a wide screen across the content alone, right of the sidebar where a page has one; 1.24.4
/// with the note going down out of sight from where it stands, on a phone with a module's sheet as
/// it closes; 1.24.5 with the start of the installed app where the system left its splash, round,
/// the wordmark at the bottom, and the page's logo and wordmark coming where they land; 1.24.6 with
/// the skeleton staying until the new page comes, not the page before coming back after it (a
/// change of tab, „Alle Studiengänge", a program's views), and a tab's mark changing in the frame of
/// the tap, without its fade or the browser's highlight; 1.25.0 with Back as an app has it on a
/// phone (what lies over a page closed first and with no entry of its own, on Android through
/// `CloseWatcher`; the semester of „Mein Studium"'s row the state of its entry; the filters' sheet
/// thrown away unless „Module anzeigen" applies it; a stack for each tab with Start under it, a
/// change of tabs writing the history in place, a link from outside laying its way up under it on
/// its first use; the arrow at the head Back) and on a desktop one step of the history per pause of
/// a second; 1.25.1 with the note of „Einplanen" on a wide screen laid out for the room it has —
/// one line, the switch a row over „Rückgängig", no name —, nothing of it over anything else;
/// 1.25.2 saying „abgeleitet" beside a kind Radix infers, „Wahlpflicht · abgeleitet", in a
/// program's list and on the module page (schema 14); 1.25.4 with the note of „Einplanen" on a wide
/// screen in order — the check and the module's name at the left, the semester, „Rückgängig" and ×
/// at the right, the switch a row over all of it, lined up with them. The version of the app's
/// crates, the same as the server's; Radix's is in the snapshot (`Meta::radix_version`).
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
/// The icon of the splash screen of the installed app, which Chromium draws at 128 dp: from a density
/// of 2.61 on it asks for more than 334 px and takes this one, never the launcher's. The maskable
/// one cut round (owner, 2026-10-18: „wenn das Icon schon am Anfang rund ist"): a browser that
/// installs a shortcut (Brave) draws it as it is, and a mask of Android's shows only its middle,
/// the full-bleed one's. Chrome's WebAPK shows Android's splash with the launcher's icon instead
/// (`HEAD_SCRIPT`). The dark
/// manifest names the turned-over one in its place, so that only the splash screen turns over in
/// the dark: an icon of the launcher that changed with the theme would make Chrome ask the visitor
/// about the app's new icon each time.
pub const ICON_SPLASH: &str = "/assets/icon-splash-512.png";
pub const ICON_SPLASH_DARK: &str = "/assets/icon-splash-dark-512.png";
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
/// system's splash screen, out of which enhance.js lets the icon fly to the page's own, and the
/// wordmark at the bottom as the start page has it (owner, 2026-10-18: „unten gerne den BeTUla
/// Schriftzug mit dem Styling wie im Start Screen"). It goes beside the body, not into it, where
/// the app takes the page over. The icon stands where the system left its own, so that nothing
/// jumps (owner, the same day: „springt der loading screen immer noch"; screen recordings of Brave
/// and Chrome on a Pixel): `--mid` its middle, `--size`, `--leaf` how large the leaf is on the
/// 96 grid and `--leaf-at` where it starts, `--name` the wordmark's baseline, all in the page.
/// - iOS: the launch screen as `launch` draws it, on the whole screen, less the status bar above
///   the window; its name is the wordmark.
/// - Chrome (an app it installs on Android is a WebAPK): Android's own splash screen, from Android
///   12 on the only one, `sys`: the launcher's icon, ink and paper whatever the theme, round,
///   240 dp, in the middle of the whole screen; no name.
/// - Brave and what else installs a shortcut: Chromium's (`SplashLayout`, `WebappSplashController`):
///   the picture of the splash screen as it is (`ICON_SPLASH`, round, the leaf at .873), 128 dp,
///   its middle 16 dp (24 on a tablet) above the middle of the whole screen up to Chromium 154,
///   and from 155 on of the screen less its bars, the gesture area of 32 dp at the bottom
///   counted; the name's baseline 54.5 dp (62.5) above that bottom.
///
/// Where the page begins under the status bar is what enhance.js measured at a tap of an earlier
/// start (`betula.launch.top`, `p` or `l` for the way the screen is held), else the bars less a
/// gesture bar of 24 dp under the page. Not on the login page of closed testing, which names no
/// manifest and runs no enhance.js. Public for that one document the server writes without the
/// app.
pub const HEAD_SCRIPT: &str = "var d=document.documentElement;d.classList.add('js');var n=new Date().getMonth();d.dataset.season=n<2||n>10?'winter':n<5?'spring':n<8?'summer':'autumn';try{var t=localStorage.getItem('betula.theme');if(t==='dark'||t==='light')d.dataset.theme=t;else t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';if(t==='dark'){var m=document.querySelector('meta[name=theme-color]');if(m)m.content='#0a0c11';m=document.querySelector('link[rel=manifest]');if(m)m.href=m.getAttribute('href').replace('/manifest.webmanifest','/manifest-dark.webmanifest')}var w=parseInt(localStorage.getItem('betula.preview.width'),10);if(w>=360&&w<=2400)d.style.setProperty('--preview-w',w+'px');var f=parseInt(localStorage.getItem('betula.filters.width'),10);if(f>=232&&f<=440)d.style.setProperty('--w-filters',f+'px');var p=/^active\\t([1-9][0-9]{0,2})$/m.exec(localStorage.getItem('betula.profiles.v1')||''),k=p&&p[1]!=='1'?'betula.p'+p[1]+'.myprogram.v1':'betula.myprogram.v1';if(/^program\\t[0-9A-Za-z]/m.test(localStorage.getItem(k)||''))d.classList.add('mine')}catch(e){}if('standalone'in navigator)(function(){var s=screen,r=Math.round(devicePixelRatio),a=Math.min(s.width,s.height)*r,b=Math.max(s.width,s.height)*r;(a<1400?[[a,b,'portrait']]:[[a,b,'portrait'],[b,a,'landscape']]).forEach(function(o){['light','dark'].forEach(function(c){var l=document.createElement('link');l.rel='apple-touch-startup-image';l.media='(orientation: '+o[2]+') and (prefers-color-scheme: '+c+')';l.href='/assets/launch/'+o[0]+'x'+o[1]+(c==='dark'?'-dark':'')+'.png';document.head.appendChild(l)})})})();(function(){try{if((navigator.standalone||matchMedia('(display-mode: standalone)').matches)&&matchMedia('(pointer: coarse)').matches&&!sessionStorage.getItem('betula.launched')){sessionStorage.setItem('betula.launched','1');if(document.querySelector('link[rel=manifest]')){var n=navigator,o=document.createElement('div'),i=n.standalone,s=screen,w=innerWidth,h=innerHeight,H=w>h?Math.min(s.width,s.height):Math.max(s.width,s.height),f=Math.max(H-h,0),q=Math.min(w,h)<600?16:24,L=q<20?54.5:62.5,z=128,k=1,c='launch',m,y,t=/^([lp])([0-9.]+)$/.exec(localStorage.getItem('betula.launch.top')||''),T=t&&t[1]==(w>h?'l':'p')&&t[2]<=f?+t[2]:Math.max(f-24,0),E=Math.max(f-T,32),v=/Chrome.([0-9]+)/.exec(n.userAgent);if(i){c+=' ios';m=H/2-f;y=H-Math.max(.075*H,44)-f}else if(!n.brave&&(n.userAgentData&&n.userAgentData.brands||[]).some(function(b){return b.brand=='Google Chrome'})){c+=' sys';z=240;m=H/2-T;y=H-Math.max(.075*H,44)-T}else{k=.873;if(v&&v[1]<155){m=H/2-q-T;y=H-L-T}else{m=(H-T-E)/2-q;y=H-E-L-T}}o.className=c;o.setAttribute('aria-hidden','true');o.style.cssText='--mid:'+m+'px;--name:'+y+'px;--size:'+z+'px;--leaf:'+z*k/96+';--leaf-at:'+z*(1-k)/2+'px';o.innerHTML='<div class=\"launch-icon\"><b></b><i></i></div><div class=\"launch-name\"><span class=\"wordmark small\"><span><span>B</span><span class=\"sc e\">E</span><span>T</span><span class=\"u\">U</span><span class=\"sc la\">LA</span></span></span></div>';d.appendChild(o)}}}catch(e){}})()";

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
