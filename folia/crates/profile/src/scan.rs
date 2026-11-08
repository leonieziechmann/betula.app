//! The scanner of „Gerät verbinden" (`/sync`, folia/design/profil/konzepte.html, the second step):
//! the camera's picture, a frame to aim with, and every code read handed to the page
//! (`on_read`), which says what it is (`folia_stores::sync::request::scanned`).
//!
//! The picture stays in the browser: the browser's own BarcodeDetector reads it where there is one
//! (Chrome on Android, Safari on a Mac), else a frame four times a second goes through
//! `folia_design::qr::read`, scaled to at most 640 pixels across. The camera is the one that looks
//! away from the person where there are two, and it is off again as soon as the scanner goes (the
//! page shows something else, or is left). Where the camera does not come — refused, none there,
//! a browser that does not give it out — the scanner says so, with the other ways (the camera app
//! opens the code's link in the browser; the recovery code).

use leptos::prelude::*;

use folia_design::ui::Icon;

use crate::i18n;

/// How the camera is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
enum Camera {
    Starting,
    On,
    /// The person or the site refused it (`NotAllowedError`, a Permissions-Policy).
    Denied,
    /// There is none (`NotFoundError`), or it is in use.
    None,
    /// The browser gives none out (an address without TLS, an old browser).
    Unsupported,
}

/// How often a frame is read, in milliseconds.
#[cfg(feature = "csr")]
const EVERY_MS: u64 = 250;

/// The widest frame read, in pixels.
#[cfg(feature = "csr")]
const WIDEST: u32 = 640;

#[component]
pub fn Scanner(#[prop(into)] on_read: Callback<String>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.scan;
    let video = NodeRef::<leptos::html::Video>::new();
    let camera = RwSignal::new(Camera::Starting);
    // „Erneut versuchen" asks for the camera again.
    let attempt = RwSignal::new(0u32);
    #[cfg(feature = "csr")]
    run(video, camera, attempt, on_read);
    #[cfg(not(feature = "csr"))]
    let _ = on_read;
    let problem = move || {
        let said = match camera.get() {
            Camera::Starting => return Some(view! { <p class="pf-cam-note" role="status">{c.scan_starting}</p> }.into_any()),
            Camera::On => return None,
            Camera::Denied => c.scan_denied,
            Camera::None => c.scan_no_camera,
            Camera::Unsupported => c.scan_unsupported,
        };
        Some(
            view! {
                <div class="pf-cam-note pf-cam-problem" role="status">
                    <Icon name="triangle-alert"/>
                    <p>{said}</p>
                    <button class="mini hit" type="button" on:click=move |_| attempt.update(|n| *n += 1)>{c.scan_retry}</button>
                </div>
            }
            .into_any(),
        )
    };
    view! {
        <div class="pf-cam" data-camera=move || format!("{:?}", camera.get()).to_lowercase()>
            <video node_ref=video playsinline="" muted="" autoplay="" prop:muted=true aria-hidden="true"></video>
            <div class="pf-aim" aria-hidden="true"></div>
            <p class="pf-cam-hint">{c.scan_hint}</p>
            {problem}
        </div>
    }
}

/// The camera and the reading, while the scanner is there.
#[cfg(feature = "csr")]
fn run(video: NodeRef<leptos::html::Video>, camera: RwSignal<Camera>, attempt: RwSignal<u32>, on_read: Callback<String>) {
    use wasm_bindgen::JsCast;

    let stream = StoredValue::new_local(None::<web_sys::MediaStream>);
    let detector = StoredValue::new_local(None::<wasm_bindgen::JsValue>);
    let canvas = StoredValue::new_local(None::<web_sys::HtmlCanvasElement>);
    let busy = StoredValue::new_local(false);
    // The last text read, so that a code held in front of the camera is handed on once.
    let last = StoredValue::new_local(String::new());
    let stop = move || {
        if let Some(stream) = stream.try_update_value(Option::take).flatten() {
            for track in stream.get_tracks().iter() {
                if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                    track.stop();
                }
            }
        }
    };
    let found = move |text: String| {
        if text.is_empty() || last.with_value(|last| *last == text) {
            return;
        }
        last.set_value(text.clone());
        on_read.run(text);
    };
    Effect::new(move |_| {
        attempt.track();
        let Some(element) = video.get() else { return };
        stop();
        camera.set(Camera::Starting);
        last.set_value(String::new());
        leptos::task::spawn_local(async move {
            match start(&element).await {
                Ok(started) => {
                    // The scanner went while the camera came.
                    if camera.try_get_untracked().is_none() {
                        stop_stream(&started);
                        return;
                    }
                    stream.set_value(Some(started));
                    detector.set_value(barcode_detector());
                    camera.set(Camera::On);
                }
                Err(kind) => {
                    camera.try_set(kind);
                }
            }
        });
    });
    let tick = move || {
        if camera.try_get_untracked() != Some(Camera::On) || busy.get_value() {
            return;
        }
        let Some(element) = video.get_untracked() else { return };
        if element.ready_state() < 2 || element.video_width() == 0 {
            return;
        }
        if let Some(reader) = detector.get_value() {
            busy.set_value(true);
            leptos::task::spawn_local(async move {
                match detect(&reader, &element).await {
                    Ok(text) => found(text.unwrap_or_default()),
                    // It cannot read here after all (a format the system lacks): the frames from now on.
                    Err(()) => detector.set_value(None),
                }
                busy.try_set_value(false);
            });
            return;
        }
        if canvas.with_value(Option::is_none) {
            canvas.set_value(web_sys::window().and_then(|w| w.document()).and_then(|d| d.create_element("canvas").ok()).and_then(|c| c.dyn_into().ok()));
        }
        let text = canvas.with_value(|canvas| canvas.as_ref().and_then(|canvas| read_frame(canvas, &element)));
        found(text.unwrap_or_default());
    };
    if let Ok(handle) = set_interval_with_handle(tick, std::time::Duration::from_millis(EVERY_MS)) {
        on_cleanup(move || {
            handle.clear();
            stop();
        });
    } else {
        on_cleanup(stop);
    }
}

#[cfg(feature = "csr")]
fn stop_stream(stream: &web_sys::MediaStream) {
    use wasm_bindgen::JsCast;
    for track in stream.get_tracks().iter() {
        if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
            track.stop();
        }
    }
}

/// The camera that looks away from the person, shown in `video`; or why there is none.
#[cfg(feature = "csr")]
async fn start(video: &web_sys::HtmlVideoElement) -> Result<web_sys::MediaStream, Camera> {
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::js_sys::{Object, Reflect};

    let devices = web_sys::window().ok_or(Camera::Unsupported)?.navigator().media_devices().map_err(|_| Camera::Unsupported)?;
    // An address without TLS has no `mediaDevices`: the binding then hands out `undefined`.
    if JsValue::from(devices.clone()).is_undefined() {
        return Err(Camera::Unsupported);
    }
    let wanted = Object::new();
    let facing = Object::new();
    let _ = Reflect::set(&facing, &"ideal".into(), &"environment".into());
    let _ = Reflect::set(&wanted, &"facingMode".into(), &facing);
    let width = Object::new();
    let _ = Reflect::set(&width, &"ideal".into(), &1280.into());
    let _ = Reflect::set(&wanted, &"width".into(), &width);
    let constraints = web_sys::MediaStreamConstraints::new();
    constraints.set_video(&wanted);
    constraints.set_audio(&false.into());
    let promise = devices.get_user_media_with_constraints(&constraints).map_err(|_| Camera::Unsupported)?;
    let stream = wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|error| {
        match Reflect::get(&error, &"name".into()).ok().and_then(|name| name.as_string()).as_deref() {
            Some("NotAllowedError") | Some("SecurityError") => Camera::Denied,
            Some("NotFoundError") | Some("OverconstrainedError") | Some("NotReadableError") | Some("AbortError") => Camera::None,
            _ => Camera::Unsupported,
        }
    })?;
    let stream: web_sys::MediaStream = stream.dyn_into().map_err(|_| Camera::Unsupported)?;
    video.set_src_object(Some(&stream));
    video.set_muted(true);
    if let Ok(playing) = video.play() {
        let _ = wasm_bindgen_futures::JsFuture::from(playing).await;
    }
    Ok(stream)
}

/// The browser's own reader of QR codes, where it has one.
#[cfg(feature = "csr")]
fn barcode_detector() -> Option<wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    use web_sys::js_sys::{Array, Function, Object, Reflect};

    let window = web_sys::window()?;
    let make = Reflect::get(&window, &"BarcodeDetector".into()).ok()?.dyn_into::<Function>().ok()?;
    let options = Object::new();
    let _ = Reflect::set(&options, &"formats".into(), &Array::of1(&"qr_code".into()));
    Reflect::construct(&make, &Array::of1(&options)).ok()
}

/// What the browser's reader finds in the video's frame now: the first code's text, `None` for
/// none; `Err` where it cannot read at all.
#[cfg(feature = "csr")]
async fn detect(detector: &wasm_bindgen::JsValue, video: &web_sys::HtmlVideoElement) -> Result<Option<String>, ()> {
    use wasm_bindgen::JsCast;
    use web_sys::js_sys::{Array, Function, Promise, Reflect};

    let call = Reflect::get(detector, &"detect".into()).ok().and_then(|f| f.dyn_into::<Function>().ok()).ok_or(())?;
    let promise = call.call1(detector, video).ok().and_then(|p| p.dyn_into::<Promise>().ok()).ok_or(())?;
    let codes = wasm_bindgen_futures::JsFuture::from(promise).await.map_err(|_| ())?;
    let first = Array::from(&codes).get(0);
    if first.is_undefined() {
        return Ok(None);
    }
    Ok(Reflect::get(&first, &"rawValue".into()).ok().and_then(|text| text.as_string()))
}

/// Reads the video's frame now through `folia_design::qr::read`, drawn on `canvas` at most
/// `WIDEST` pixels across.
#[cfg(feature = "csr")]
fn read_frame(canvas: &web_sys::HtmlCanvasElement, video: &web_sys::HtmlVideoElement) -> Option<String> {
    use wasm_bindgen::JsCast;

    let (width, height) = (video.video_width(), video.video_height());
    if width == 0 || height == 0 {
        return None;
    }
    let scale = (f64::from(WIDEST) / f64::from(width)).min(1.0);
    let (w, h) = ((f64::from(width) * scale).round() as u32, (f64::from(height) * scale).round() as u32);
    canvas.set_width(w);
    canvas.set_height(h);
    let context = canvas.get_context("2d").ok()??.dyn_into::<web_sys::CanvasRenderingContext2d>().ok()?;
    context.draw_image_with_html_video_element_and_dw_and_dh(video, 0.0, 0.0, f64::from(w), f64::from(h)).ok()?;
    let rgba = context.get_image_data(0.0, 0.0, f64::from(w), f64::from(h)).ok()?.data();
    let luma: Vec<u8> = rgba.chunks_exact(4).map(luma_of).collect();
    folia_design::qr::read(&luma, w as usize, h as usize)
}

/// A pixel's brightness (ITU-R BT.601, in integers).
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
fn luma_of(px: &[u8]) -> u8 {
    let [r, g, b] = [0, 1, 2].map(|i| u32::from(px.get(i).copied().unwrap_or(255)));
    u8::try_from((r * 77 + g * 150 + b * 29) >> 8).unwrap_or(255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_has_its_brightness() {
        assert_eq!(luma_of(&[0, 0, 0, 255]), 0);
        assert_eq!(luma_of(&[255, 255, 255, 255]), 255);
        assert_eq!(luma_of(&[255, 0, 0, 255]), 76);
        assert!(luma_of(&[0, 255, 0, 255]) > luma_of(&[0, 0, 255, 255]));
    }
}
