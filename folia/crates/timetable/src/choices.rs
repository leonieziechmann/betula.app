//! The choices among a module's events, of which a student attends some options, not all, and its
//! extras (folia/design/wahlangebote/konzept.html; docs/radix/schema-v2.md, „Choices"; owner,
//! 2026-10-19: „ich will nicht, dass wir sonderfälle per hand eintragen, sondern eine lösung haben,
//! die in die zukunft funktioniert"). Radix reads them at every build from the SWS of a module and
//! of its events, their forms, their titles and whom QIS lists each event for; here only what the
//! pages make of it: the choices of a semester, their options, whom each is for, and which
//! options a program takes. An extra is a date's own (`EventDate::extra`).

use std::collections::BTreeSet;

use folia_model::labels::{Campus, ChoiceBasis, ChoiceKind};
use folia_model::rows_detail::{ChoiceFamily, ChoiceOption};

/// The choices of a module in one semester, in their order.
#[derive(Clone, Debug, PartialEq)]
pub struct Choices {
    pub list: Vec<Choice>,
}

/// A choice among a module's events: of its options one takes `pick`.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// Its number in the semester (`v_module_choice.choice`).
    pub number: u32,
    /// The course at a campus it is in, where the module has courses (`EventDate::track`).
    pub course: Option<Campus>,
    /// What it offers; a kind a newer Radix writes reads as a topic, the most general one.
    pub kind: ChoiceKind,
    pub basis: Option<ChoiceBasis>,
    /// What the options taken cover; `None` for groups a title names.
    pub sws: Option<f64>,
    pub pick: u32,
    /// In the order of their numbers.
    pub options: Vec<Opt>,
}

/// An option of a choice: one event or more („Gruppe 2" of two titles).
#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    pub number: u32,
    /// Its events, by id.
    pub events: Vec<String>,
    /// „Gruppe 2", „A-Kurs", where a title names the group.
    pub label: Option<String>,
    /// The SWS of its events together, where QIS gives each of them some.
    pub sws: Option<f64>,
    /// The families of the programs QIS lists at its events.
    pub families: BTreeSet<String>,
    /// Their names, one per family (its newest PO's), each once and in the order of the names:
    /// whom the option is for.
    pub listed: Vec<String>,
}

/// How big the options of a choice are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Every option QIS gives SWS for has these.
    Each(f64),
    /// They differ (English for Academic Purposes: 4, 4, 2, 2 for 6 SWS).
    Differ,
    /// QIS gives none of them any.
    Unknown,
}

impl Choices {
    /// The choices of `semester`, from the rows of `v_module_choice` and `v_module_choice_family`;
    /// `None` where it has none.
    pub fn of(rows: &[ChoiceOption], families: &[ChoiceFamily], semester: &str) -> Option<Choices> {
        let mut list: Vec<Choice> = Vec::new();
        for row in rows.iter().filter(|row| row.semester_key == semester) {
            let at = match list.iter().position(|choice| choice.number == row.choice) {
                Some(at) => at,
                None => {
                    list.push(Choice {
                        number: row.choice,
                        course: row.course.as_ref().and_then(|course| course.known()),
                        kind: row.kind.known().unwrap_or(ChoiceKind::Topic),
                        basis: row.basis.known(),
                        sws: row.sws,
                        pick: row.pick.max(1),
                        options: Vec::new(),
                    });
                    list.len() - 1
                }
            };
            let Some(choice) = list.get_mut(at) else { continue };
            let option = match choice.options.iter().position(|option| option.number == row.option) {
                Some(at) => at,
                None => {
                    choice.options.push(Opt { number: row.option, events: Vec::new(), label: None, sws: Some(0.0), families: BTreeSet::new(), listed: Vec::new() });
                    choice.options.len() - 1
                }
            };
            let Some(option) = choice.options.get_mut(option) else { continue };
            if !option.events.contains(&row.event_id) {
                option.events.push(row.event_id.clone());
                // An event without SWS leaves the option's size unknown.
                option.sws = option.sws.zip(row.event_sws.filter(|sws| *sws > 0.0)).map(|(sum, sws)| sum + sws);
            }
            if option.label.is_none() {
                option.label = row.label.clone().filter(|label| !label.trim().is_empty());
            }
        }
        if list.is_empty() {
            return None;
        }
        for choice in &mut list {
            choice.options.sort_by_key(|option| option.number);
            for option in &mut choice.options {
                let rows: Vec<&ChoiceFamily> =
                    families.iter().filter(|row| row.semester_key == semester && row.choice == choice.number && row.option == option.number).collect();
                option.families = rows.iter().map(|row| row.family_key.clone()).collect();
                // One name per family: its newest PO's, else the first.
                let mut names: Vec<String> = option
                    .families
                    .iter()
                    .filter_map(|family| {
                        let of: Vec<&&ChoiceFamily> = rows.iter().filter(|row| row.family_key == *family).collect();
                        of.iter().find(|row| row.is_latest_po).or(of.first()).map(|row| row.program_name.trim().to_string())
                    })
                    .filter(|name| !name.is_empty())
                    .collect();
                names.sort_by_key(|name| name.to_lowercase());
                names.dedup();
                option.listed = names;
            }
        }
        list.sort_by_key(|choice| choice.number);
        Some(Choices { list })
    }

    /// The choice and the option an event is one of.
    pub fn of_event(&self, event_id: &str) -> Option<(&Choice, &Opt)> {
        self.list.iter().find_map(|choice| choice.options.iter().find(|option| option.events.iter().any(|event| event == event_id)).map(|option| (choice, option)))
    }
}

impl Choice {
    /// The options QIS lists `family` at, where it lists it at some but not at every one: those a
    /// program of the family takes. `None` where QIS lists it at none or at each.
    pub fn listed_for(&self, family: &str) -> Option<BTreeSet<u32>> {
        let own: BTreeSet<u32> = self.options.iter().filter(|option| option.families.contains(family)).map(|option| option.number).collect();
        (!own.is_empty() && own.len() < self.options.len()).then_some(own)
    }

    /// Whether QIS lists `family` at every option.
    pub fn listed_at_each(&self, family: &str) -> bool {
        self.options.iter().all(|option| option.families.contains(family))
    }

    /// How big its options are.
    pub fn size(&self) -> Size {
        let sizes: Vec<f64> = self.options.iter().filter_map(|option| option.sws).collect();
        match sizes.split_first() {
            None => Size::Unknown,
            Some((first, rest)) if rest.iter().all(|size| (size - first).abs() < 0.01) => Size::Each(*first),
            Some(_) => Size::Differ,
        }
    }

    /// Whether QIS lists other programs at its options: not the same families at each.
    pub fn split_by_program(&self) -> bool {
        self.options.windows(2).any(|pair| matches!(pair, [a, b] if a.families != b.families))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_model::labels::Code;

    #[allow(clippy::too_many_arguments)]
    fn option(choice: u32, kind: &str, basis: &str, sws: Option<f64>, pick: u32, option: u32, event: &str, label: Option<&str>, event_sws: Option<f64>) -> ChoiceOption {
        ChoiceOption {
            module_id: "11206".into(),
            semester_key: "2026W".into(),
            choice,
            course: Some(Code::parse("zentralcampus")),
            kind: Code::parse(kind),
            basis: Code::parse(basis),
            sws,
            pick,
            option,
            event_id: event.into(),
            label: label.map(str::to_string),
            event_sws,
        }
    }

    fn family(choice: u32, option: u32, family: &str, name: &str, latest: bool) -> ChoiceFamily {
        ChoiceFamily {
            module_id: "11206".into(),
            semester_key: "2026W".into(),
            choice,
            option,
            family_key: family.into(),
            program_name: name.into(),
            is_latest_po: latest,
        }
    }

    /// Höhere Mathematik T3 at the Zentralcampus in 2026W: three exercises, each for other programs.
    #[test]
    fn the_exercises_of_hm_t3() {
        let rows = [
            option(1, "program", "form", Some(2.0), 1, 1, "148245", None, Some(2.0)),
            option(1, "program", "form", Some(2.0), 1, 2, "148246", None, Some(2.0)),
            option(1, "program", "form", Some(2.0), 1, 3, "148932", None, Some(2.0)),
        ];
        let families = [
            family(1, 1, "048-82", "Elektrotechnik", true),
            family(1, 1, "774-82", "Umweltingenieurwesen", false),
            family(1, 1, "774-82", "Umweltingenieurwesen ", true),
            family(1, 2, "017-82", "Bauingenieurwesen", true),
            family(1, 3, "104-82", "Maschinenbau", true),
            family(1, 3, "774-82", "Umweltingenieurwesen", true),
        ];
        let choices = Choices::of(&rows, &families, "2026W").expect("a choice");
        let [choice] = choices.list.as_slice() else { panic!("{choices:?}") };
        assert_eq!((choice.kind, choice.course, choice.pick, choice.options.len()), (ChoiceKind::Program, Some(Campus::Zentralcampus), 1, 3));
        assert_eq!(choice.options[0].listed, ["Elektrotechnik", "Umweltingenieurwesen"]);
        assert_eq!(choice.size(), Size::Each(2.0));
        assert!(choice.split_by_program());
        // Bauingenieurwesen takes the one exercise QIS lists it at, Umweltingenieurwesen chooses
        // between two, a program QIS lists at none takes none.
        assert_eq!(choice.listed_for("017-82"), Some(BTreeSet::from([2])));
        assert_eq!(choice.listed_for("774-82"), Some(BTreeSet::from([1, 3])));
        assert_eq!(choice.listed_for("079-82"), None);
        assert_eq!(choices.of_event("148246").map(|(choice, option)| (choice.number, option.number)), Some((1, 2)));
        assert_eq!(choices.of_event("148060"), None);
        assert_eq!(Choices::of(&rows, &families, "2027S"), None);
    }

    /// Musiktheorie: groups of two events each; English for Academic Purposes: courses of 4 and 2
    /// SWS; an option without SWS in QIS.
    #[test]
    fn groups_and_sizes() {
        let groups = [
            option(1, "group", "named", None, 1, 1, "149750", Some("Gruppe 1"), Some(2.0)),
            option(1, "group", "named", None, 1, 1, "149753", Some("Gruppe 1"), Some(1.0)),
            option(1, "group", "named", None, 1, 2, "149752", Some("Gruppe 2"), Some(2.0)),
            option(1, "group", "named", None, 1, 2, "149755", Some("Gruppe 2"), Some(1.0)),
        ];
        let choices = Choices::of(&groups, &[], "2026W").expect("a choice");
        let choice = &choices.list[0];
        assert_eq!(choice.options[0].events, ["149750", "149753"]);
        assert_eq!(choice.options[1].label.as_deref(), Some("Gruppe 2"));
        assert_eq!(choice.size(), Size::Each(3.0));
        assert!(!choice.split_by_program());
        let courses = [
            option(1, "topic", "form", Some(6.0), 2, 1, "1", None, Some(4.0)),
            option(1, "topic", "form", Some(6.0), 2, 2, "2", None, Some(2.0)),
            option(1, "topic", "form", Some(6.0), 2, 3, "3", None, None),
        ];
        let choice = &Choices::of(&courses, &[], "2026W").expect("a choice").list[0];
        assert_eq!((choice.options[2].sws, choice.size()), (None, Size::Differ));
        let sizeless = [option(1, "future", "form", None, 1, 1, "1", None, None), option(1, "future", "form", None, 1, 2, "2", None, None)];
        let choice = &Choices::of(&sizeless, &[], "2026W").expect("a choice").list[0];
        assert_eq!((choice.kind, choice.size()), (ChoiceKind::Topic, Size::Unknown));
    }
}
