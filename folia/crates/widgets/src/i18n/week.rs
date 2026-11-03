//! Texts of a week of Termine as a grid (`week.rs`). The grid writes few words of its own: the
//! heads of its days are the weekdays of the data (`folia_locale::common`), and what its slots
//! say comes from the page that draws them. Its own are the times of a slot of the plan beside a
//! module's week that the week's frame cuts (folia/design/kalender/raster.html): until when it
//! goes on under the week, since when in its box, and what lies wholly after the frame.

pub struct Texts {
    /// Under a slot cut at the frame's end: „bis 17:00".
    pub until: fn(&str) -> String,
    /// In a slot cut at the frame's start: „ab 07:30".
    pub since: fn(&str) -> String,
    /// Under a day, for a slot wholly after the frame: „+ ABWL1 17:30".
    pub later: fn(&str, &str) -> String,
}

pub const DE: Texts = Texts {
    until: |time| format!("bis {time}"),
    since: |time| format!("ab {time}"),
    later: |name, time| format!("+ {name} {time}"),
};

pub const EN: Texts = Texts {
    until: |time| format!("until {time}"),
    since: |time| format!("from {time}"),
    later: |name, time| format!("+ {name} {time}"),
};
