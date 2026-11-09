//! Texts of the devices of a synced profile wherever one is named (`sync::devices`).

pub struct Texts {
    /// A device of another kind without a model the browser told: „Handy", „Tablet", „Rechner".
    pub phone: &'static str,
    pub tablet: &'static str,
    pub computer: &'static str,
    /// This device: „Dieses Handy", „Dieses Tablet", „Dieser Rechner".
    pub this_phone: &'static str,
    pub this_tablet: &'static str,
    pub this_computer: &'static str,
    /// Betula installed on the home screen, in place of a browser's name.
    pub app: &'static str,
}

pub const DE: Texts = Texts { phone: "Handy", tablet: "Tablet", computer: "Rechner", this_phone: "Dieses Handy", this_tablet: "Dieses Tablet", this_computer: "Dieser Rechner", app: "App" };

pub const EN: Texts = Texts { phone: "Phone", tablet: "Tablet", computer: "Computer", this_phone: "This phone", this_tablet: "This tablet", this_computer: "This computer", app: "App" };
