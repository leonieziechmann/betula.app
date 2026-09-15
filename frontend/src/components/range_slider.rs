use leptos::prelude::*;

/// Generic Dual Range Slider with visual active track, min and max handles, and scale ticks
#[component]
pub fn DualRangeSlider(
    #[prop(default = 0.0)] min_bound: f64,
    #[prop(default = 30.0)] max_bound: f64,
    #[prop(default = 1.0)] step: f64,
    min_val: Signal<f64>,
    max_val: Signal<f64>,
    on_change: Callback<(f64, f64)>,
    #[prop(default = vec!["0", "6", "12", "18", "24", "30"])] scale_marks: Vec<&'static str>,
) -> impl IntoView {
    let range_span = (max_bound - min_bound).max(1.0);

    let track_style = move || {
        let cur_min = min_val.get();
        let cur_max = max_val.get();
        let left = ((cur_min - min_bound) / range_span) * 100.0;
        let right = ((max_bound - cur_max) / range_span) * 100.0;
        format!("left: {:.2}%; right: {:.2}%;", left.clamp(0.0, 100.0), right.clamp(0.0, 100.0))
    };

    let on_min_input = {
        let on_change = on_change;
        move |ev| {
            let val: f64 = event_target_value(&ev).parse().unwrap_or(min_bound);
            let cur_max = max_val.get();
            if val <= cur_max {
                on_change.run((val, cur_max));
            }
        }
    };

    let on_max_input = {
        let on_change = on_change;
        move |ev| {
            let val: f64 = event_target_value(&ev).parse().unwrap_or(max_bound);
            let cur_min = min_val.get();
            if val >= cur_min {
                on_change.run((cur_min, val));
            }
        }
    };

    view! {
        <div class="dual-slider-container">
            <div class="dual-slider-track" style=track_style></div>
            <input
                type="range"
                min=min_bound
                max=max_bound
                step=step
                prop:value=min_val
                class="dual-range-input dual-range-min"
                on:input=on_min_input
            />
            <input
                type="range"
                min=min_bound
                max=max_bound
                step=step
                prop:value=max_val
                class="dual-range-input dual-range-max"
                on:input=on_max_input
            />
        </div>
        <div class="slider-scale">
            {scale_marks.into_iter().map(|mark| {
                view! { <span>{mark}</span> }
            }).collect::<Vec<_>>()}
        </div>
    }
}
