//! The lanes and the span of a week grid.
//!
//! A week grid (the module page's, the Studienplan's Regelwoche) draws each slot at its day and
//! times. Slots that overlap on a day are put side by side instead of on top of one another: each
//! gets a lane, and every slot of a group of overlapping slots knows how many lanes the group
//! needs, so the page can size them alike. The span says which days and hours the grid needs. Pure
//! arithmetic on minutes; the app turns minutes into its CSS unit.

/// A slot of the grid: weekday (1 = Monday) and minutes since midnight, `to > from`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub day: u8,
    pub from: u16,
    pub to: u16,
}

/// Per input slot, in input order: its lane (0 = leftmost) and the number of lanes of its group.
///
/// Per day, the slots are taken by start, the longer first at the same start, then by input order;
/// a group is a run of slots that overlap one another transitively (a slot that starts when
/// another ends does not overlap it); each slot takes the lowest lane that is free at its start,
/// and the group's lanes are as many as it used.
pub fn lanes(slots: &[Placed]) -> Vec<(u8, u8)> {
    let mut result = vec![(0u8, 1u8); slots.len()];
    let mut order: Vec<(usize, &Placed)> = slots.iter().enumerate().collect();
    order.sort_by_key(|(i, s)| (s.day, s.from, std::cmp::Reverse(s.to.saturating_sub(s.from)), *i));

    let mut group: Vec<usize> = Vec::new();
    // The end of the last slot in each lane of the group.
    let mut lane_ends: Vec<u16> = Vec::new();
    let mut group_day = None;
    let mut group_end = 0u16;
    for (i, slot) in order {
        let end = slot.to.max(slot.from);
        if group_day != Some(slot.day) || slot.from >= group_end {
            close(&group, lane_ends.len(), &mut result);
            group.clear();
            lane_ends.clear();
            group_day = Some(slot.day);
            group_end = end;
        }
        let lane = match lane_ends.iter_mut().enumerate().find(|(_, lane_end)| **lane_end <= slot.from) {
            Some((lane, lane_end)) => {
                *lane_end = end;
                lane
            }
            None => {
                lane_ends.push(end);
                lane_ends.len() - 1
            }
        };
        if let Some(entry) = result.get_mut(i) {
            entry.0 = saturated(lane);
        }
        group_end = group_end.max(end);
        group.push(i);
    }
    close(&group, lane_ends.len(), &mut result);
    result
}

/// Writes a finished group's lane count into each of its slots.
fn close(group: &[usize], lanes: usize, result: &mut [(u8, u8)]) {
    for i in group {
        if let Some(entry) = result.get_mut(*i) {
            entry.1 = saturated(lanes);
        }
    }
}

fn saturated(n: usize) -> u8 {
    u8::try_from(n).unwrap_or(u8::MAX)
}

/// The frame a grid needs: its days and hours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// 5 (Monday to Friday), 6 with a Saturday slot, 7 with a Sunday slot.
    pub days: u8,
    /// The first hour drawn, in minutes: the earliest start floored to the hour, 08:00 at the
    /// latest.
    pub first: u16,
    /// The last hour drawn, in minutes: the latest end ceiled to the hour, at least
    /// `first + min_minutes`, at most 24:00.
    pub last: u16,
}

/// The days and hours the slots need. Without any slot it is the frame of the rule (Monday to
/// Friday from 08:00 for `min_minutes`); whether to draw that is the caller's call.
pub fn span(slots: &[Placed], min_minutes: u16) -> Span {
    const DAY_END: u16 = 24 * 60;
    const LATEST_FIRST: u16 = 8 * 60;
    let days = if slots.iter().any(|s| s.day == 7) {
        7
    } else if slots.iter().any(|s| s.day == 6) {
        6
    } else {
        5
    };
    let first = slots.iter().map(|s| s.from).min().map_or(LATEST_FIRST, |from| (from / 60 * 60).min(LATEST_FIRST));
    let latest_end = slots.iter().map(|s| s.to.max(s.from)).max().unwrap_or(0);
    let ceiled = u16::try_from(u32::from(latest_end).div_ceil(60) * 60).unwrap_or(u16::MAX);
    let last = ceiled.max(first.saturating_add(min_minutes)).min(DAY_END);
    Span { days, first, last }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timetable::day::minutes;

    fn at(day: u8, from: &str, to: &str) -> Placed {
        Placed { day, from: minutes(from).unwrap(), to: minutes(to).unwrap() }
    }

    #[test]
    fn overlapping_slots_go_side_by_side() {
        let three = [at(2, "08:00", "10:00"), at(2, "08:30", "09:30"), at(2, "09:00", "11:00")];
        assert_eq!(lanes(&three), [(0, 3), (1, 3), (2, 3)]);
        // A chain A–B, B–C: one group, but A and C share a lane.
        let chain = [at(1, "08:00", "10:00"), at(1, "09:00", "11:00"), at(1, "10:30", "12:00")];
        assert_eq!(lanes(&chain), [(0, 2), (1, 2), (0, 2)]);
        // Touching ends do not overlap.
        let touching = [at(1, "08:00", "10:00"), at(1, "10:00", "12:00")];
        assert_eq!(lanes(&touching), [(0, 1), (0, 1)]);
        // Other days never share a group.
        let days = [at(1, "08:00", "10:00"), at(2, "08:00", "10:00")];
        assert_eq!(lanes(&days), [(0, 1), (0, 1)]);
        // The longer slot takes the left lane at the same start, whatever the input order.
        let same_start = [at(3, "09:15", "10:45"), at(3, "09:15", "12:30")];
        assert_eq!(lanes(&same_start), [(1, 2), (0, 2)]);
        assert!(lanes(&[]).is_empty());
    }

    /// Module 12229 in WiSe 2026/27: on Tuesday the single date of its Vorlesung (11:45–15:00)
    /// and the Übung of the 2-Gruppe (13:45–15:15) overlap.
    #[test]
    fn the_tuesday_of_12229() {
        let slots = [
            at(2, "11:45", "15:00"),
            at(4, "11:30", "13:00"),
            at(1, "13:45", "15:15"),
            at(2, "13:45", "15:15"),
        ];
        assert_eq!(lanes(&slots), [(0, 2), (0, 1), (0, 1), (1, 2)]);
        assert_eq!(span(&slots, 240), Span { days: 5, first: 480, last: 960 });
    }

    #[test]
    fn the_span_of_the_week() {
        assert_eq!(span(&[at(6, "10:00", "12:00")], 240).days, 6);
        assert_eq!(span(&[at(7, "10:00", "12:00"), at(6, "10:00", "12:00")], 240).days, 7);
        assert_eq!(span(&[at(1, "07:30", "09:00")], 0).first, 420);
        assert_eq!(span(&[at(1, "09:15", "10:45")], 0), Span { days: 5, first: 480, last: 660 });
        assert_eq!(span(&[at(5, "22:00", "24:00")], 0).last, 1440);
        assert_eq!(span(&[at(5, "09:00", "10:00")], 240).last, 720);
        // No slot: the frame of the rule, no panic.
        assert_eq!(span(&[], 240), Span { days: 5, first: 480, last: 720 });
        assert_eq!(span(&[], u16::MAX), Span { days: 5, first: 480, last: 1440 });
        // A slot that ends before it starts is taken as the point of its start: no panic, and a
        // lane beside what it falls into.
        assert_eq!(lanes(&[Placed { day: 1, from: 600, to: 500 }, at(1, "09:00", "10:30")]), [(1, 2), (0, 2)]);
        assert_eq!(lanes(&[Placed { day: 1, from: 630, to: 500 }, at(1, "09:00", "10:30")]), [(0, 1), (0, 1)]);
    }
}
