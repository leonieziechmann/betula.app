use leptos::prelude::*;
use crate::models::FilterOptions;

/// Turnus matrix selection component for WiSe/SoSe (gerade/ungerade) and Sporadisch
#[component]
pub fn TurnusMatrix(
    filters: RwSignal<FilterOptions>,
) -> impl IntoView {
    view! {
        <table class="turnus-matrix-table">
            <thead>
                <tr>
                    <th style="width: 38%;"></th>
                    <th style="width: 31%;">"Gerade"</th>
                    <th style="width: 31%;">"Ungerade"</th>
                </tr>
            </thead>
            <tbody>
                <tr>
                    <td>
                        <button
                            type="button"
                            class="btn-turnus-row"
                            on:click=move |_| filters.update(|f| {
                                f.turnus_all = false;
                                f.turnus_next = false;
                                let state = !(f.turnus_wise_even && f.turnus_wise_odd);
                                f.turnus_wise_even = state;
                                f.turnus_wise_odd = state;
                            })
                        >
                            "❄️ WiSe"
                        </button>
                    </td>
                    <td class="text-center">
                        <label class="turnus-matrix-cell">
                            <input
                                type="checkbox"
                                prop:checked=move || filters.get().turnus_wise_even
                                on:change=move |ev| {
                                    let chk = event_target_checked(&ev);
                                    filters.update(|f| {
                                        f.turnus_all = false;
                                        f.turnus_next = false;
                                        f.turnus_wise_even = chk;
                                    });
                                }
                            />
                            <span class="custom-checkbox"></span>
                        </label>
                    </td>
                    <td class="text-center">
                        <label class="turnus-matrix-cell">
                            <input
                                type="checkbox"
                                prop:checked=move || filters.get().turnus_wise_odd
                                on:change=move |ev| {
                                    let chk = event_target_checked(&ev);
                                    filters.update(|f| {
                                        f.turnus_all = false;
                                        f.turnus_next = false;
                                        f.turnus_wise_odd = chk;
                                    });
                                }
                            />
                            <span class="custom-checkbox"></span>
                        </label>
                    </td>
                </tr>
                <tr>
                    <td>
                        <button
                            type="button"
                            class="btn-turnus-row"
                            on:click=move |_| filters.update(|f| {
                                f.turnus_all = false;
                                f.turnus_next = false;
                                let state = !(f.turnus_sose_even && f.turnus_sose_odd);
                                f.turnus_sose_even = state;
                                f.turnus_sose_odd = state;
                            })
                        >
                            "☀️ SoSe"
                        </button>
                    </td>
                    <td class="text-center">
                        <label class="turnus-matrix-cell">
                            <input
                                type="checkbox"
                                prop:checked=move || filters.get().turnus_sose_even
                                on:change=move |ev| {
                                    let chk = event_target_checked(&ev);
                                    filters.update(|f| {
                                        f.turnus_all = false;
                                        f.turnus_next = false;
                                        f.turnus_sose_even = chk;
                                    });
                                }
                            />
                            <span class="custom-checkbox"></span>
                        </label>
                    </td>
                    <td class="text-center">
                        <label class="turnus-matrix-cell">
                            <input
                                type="checkbox"
                                prop:checked=move || filters.get().turnus_sose_odd
                                on:change=move |ev| {
                                    let chk = event_target_checked(&ev);
                                    filters.update(|f| {
                                        f.turnus_all = false;
                                        f.turnus_next = false;
                                        f.turnus_sose_odd = chk;
                                    });
                                }
                            />
                            <span class="custom-checkbox"></span>
                        </label>
                    </td>
                </tr>
            </tbody>
        </table>
        <div class="turnus-sporadic-wrapper">
            <label class="turnus-sporadic-label">
                <input
                    type="checkbox"
                    prop:checked=move || filters.get().turnus_sporadic
                    on:change=move |ev| {
                        let chk = event_target_checked(&ev);
                        filters.update(|f| {
                            f.turnus_all = false;
                            f.turnus_next = false;
                            f.turnus_sporadic = chk;
                        });
                    }
                />
                <span class="custom-checkbox"></span>
                <span>"🎲 Sporadisch"</span>
            </label>
        </div>
    }
}
