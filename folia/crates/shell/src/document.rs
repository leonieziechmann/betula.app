//! What the document of every page links and runs: where the host serves the files of
//! `folia/assets`, the colours of the browser's own chrome, and the script before the first paint.

/// The release of Folia as the owner names it (2026-09-21: Folia and Radix are both
/// alpha-0.2.0; 2026-09-22: Folia alpha-0.2.1 with the phone's filter sheet; 2026-09-23:
/// Folia alpha-0.2.2, Radix alpha-0.3.0; 2026-09-27, after the public release: Folia 1.0.1,
/// Radix 0.5.0, no stage in front any more; 2026-09-29: Folia 1.0.2, the same day 1.0.3 with the
/// wood behind every page): the version of the app's crates, the same as the server's.
/// Radix's is in the snapshot (`Meta::radix_version`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the host serves the files of `folia/assets`.
pub const STYLESHEET: &str = "/assets/app.css";
pub const FAVICON: &str = "/assets/favicon.svg";
/// The pictures for what cannot read the SVG (`design/logo/render-icons.mjs` makes them): the
/// classic `/favicon.ico`, the mark like the SVG; and the icon of the installed app, the birch leaf
/// that carries the mark's bars (`design/logo/app-icon.mjs`): the icon of iOS (home screen, link
/// previews of Messages) and the icons of the web app manifest, plain ones for desktops, maskable
/// ones for Android's launchers and the splash screen of the installed app (the large one keeps it
/// sharp there), and the monochrome one that Android's themed icons tint in the colours of the
/// wallpaper. The site keeps its mark; the leaf stands whole in whatever shape a launcher cuts.
pub const FAVICON_ICO: &str = "/favicon.ico";
pub const TOUCH_ICON: &str = "/apple-touch-icon.png";
/// Also the picture Google Search shows beside the site's results (owner, 2026-10-01: the icon of
/// the app there). Google takes one per host from the start page's `icon` and `apple-touch-icon`
/// links, reads no SVG and asks for a square larger than 48 px; how it chooses among several it
/// does not document (as far as can be seen, the largest it reads). Linked as an `icon` of 192 px,
/// this one is the largest of either kind. The tab keeps the mark: Chromium and Firefox take the
/// SVG whatever else is linked (Chromium does not even fetch this one).
pub const ICON_192: &str = "/assets/icon-192.png";
pub const ICON_512: &str = "/assets/icon-512.png";
pub const ICON_MASKABLE: &str = "/assets/icon-maskable-512.png";
pub const ICON_MASKABLE_LARGE: &str = "/assets/icon-maskable-1024.png";
pub const ICON_MONOCHROME: &str = "/assets/icon-monochrome-512.png";
/// Name, colours and icons of the site for a home screen or an installed window.
pub const MANIFEST: &str = "/manifest.webmanifest";
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
/// colour of the browser's own chrome (`theme-color`, `THEME_DARK`) follows the theme. Such personal
/// view settings live in localStorage, never in the URL and never in server HTML (R9). A browser
/// that keeps „Mein Studiengang" marks the document with `mine`: the program overview's line about
/// it is the app's, and the page keeps its room from the first paint, so the list does not move
/// when the app takes over (R15). On an iPhone or iPad (`navigator.standalone` exists there only)
/// it names the launch screens of this screen, upright and, on a tablet, turned, light and dark
/// (`launch`): iOS takes them when the app is added to the home screen, from the page as it is
/// then. Written here and not as sixty `<link>`s into every page, which would cost every visitor
/// almost a kilobyte for what only a home screen of iOS reads. Public for the one document the
/// server writes without the app: the login page of closed testing.
pub const HEAD_SCRIPT: &str = "var d=document.documentElement;d.classList.add('js');var n=new Date().getMonth();d.dataset.season=n<2||n>10?'winter':n<5?'spring':n<8?'summer':'autumn';try{var t=localStorage.getItem('betula.theme');if(t==='dark'||t==='light')d.dataset.theme=t;else t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light';if(t==='dark'){var m=document.querySelector('meta[name=theme-color]');if(m)m.content='#0a0c11'}var w=parseInt(localStorage.getItem('betula.preview.width'),10);if(w>=360&&w<=2400)d.style.setProperty('--preview-w',w+'px');var f=parseInt(localStorage.getItem('betula.filters.width'),10);if(f>=232&&f<=440)d.style.setProperty('--w-filters',f+'px');if(/^program\\t[0-9A-Za-z]/m.test(localStorage.getItem('betula.myprogram.v1')||''))d.classList.add('mine')}catch(e){}if('standalone'in navigator)(function(){var s=screen,r=Math.round(devicePixelRatio),a=Math.min(s.width,s.height)*r,b=Math.max(s.width,s.height)*r;(a<1400?[[a,b,'portrait']]:[[a,b,'portrait'],[b,a,'landscape']]).forEach(function(o){['light','dark'].forEach(function(c){var l=document.createElement('link');l.rel='apple-touch-startup-image';l.media='(orientation: '+o[2]+') and (prefers-color-scheme: '+c+')';l.href='/assets/launch/'+o[0]+'x'+o[1]+(c==='dark'?'-dark':'')+'.png';document.head.appendChild(l)})})})()";

/// The opt-in to the fade between pages, in the head of every document the server writes (this
/// shell and the login page of closed testing). Not in app.css: Chromium decides whether a new
/// page takes part in the transition when it shows it for the first time, from the style sheets it
/// has applied by then, and the stylesheet, revalidated on every page load, often arrives after the
/// parser has reached `<body>`. The page then came without the fade and with "Transition was
/// aborted because of invalid state. ViewTransition opt-in disabled" in the console. Written
/// inline, the rule is there before the body is. The duration stays in app.css.
pub const VIEW_TRANSITION_STYLE: &str = "@view-transition{navigation:auto}";
