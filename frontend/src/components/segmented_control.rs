use leptos::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct SegmentedOption<T> {
    pub value: T,
    pub label: String,
}

impl<T> SegmentedOption<T> {
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

/// Generic Segmented Control / Radio Button Selector
#[component]
pub fn SegmentedControl<T>(
    options: Vec<SegmentedOption<T>>,
    selected: Signal<T>,
    on_select: Callback<T>,
    #[prop(optional, into)] container_class: Option<String>,
    #[prop(optional, into)] button_class: Option<String>,
) -> impl IntoView
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    let container_cls = container_class.unwrap_or_else(|| "segmented-control".to_string());
    let btn_base_cls = button_class.unwrap_or_else(|| "segmented-btn".to_string());

    view! {
        <div class=container_cls>
            {options.into_iter().map(|opt| {
                let opt_val = opt.value.clone();
                let opt_val_click = opt.value.clone();
                let btn_cls = {
                    let btn_base = btn_base_cls.clone();
                    let opt_val = opt_val.clone();
                    move || {
                        if selected.get() == opt_val {
                            format!("{} active", btn_base)
                        } else {
                            btn_base.clone()
                        }
                    }
                };
                view! {
                    <button
                        type="button"
                        class=btn_cls
                        on:click=move |_| on_select.run(opt_val_click.clone())
                    >
                        {opt.label}
                    </button>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}
