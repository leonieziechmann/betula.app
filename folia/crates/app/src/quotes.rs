//! What the texts of one feature quote of another's (a button, a view, a label): the features
//! know nothing of each other, so the app, which has all of them, checks that they match.

#[cfg(test)]
mod tests {
    use folia_locale::Locale;
    use folia_routes::url::ProgramTab;

    use crate::i18n;

    /// What the answers quote of the page (a button, a view, a filter, a section) is what the page
    /// says there, in every language: renamed there, it has to be renamed here.
    #[test]
    fn the_answers_quote_the_page_as_it_is() {
        for locale in Locale::ALL {
            let t = i18n::texts(*locale);
            let (open, close) = if *locale == Locale::De { ("„", "“") } else { ("“", "”") };
            let quoted = |label: &str| format!("{open}{label}{close}");
            let home = &t.home;
            let answer = |question: usize, faq: &folia_home::i18n::home::Faq| faq.questions.get(question).map(|(_, answer)| *answer).unwrap_or_default();
            let sync = &folia_profile::i18n::texts(*locale).sync;
            let pairs = [
                (answer(0, &home.using_betula), quoted(t.topics.finding.name)),
                (answer(0, &home.using_betula), quoted(t.topics.more)),
                (answer(1, &home.using_betula), quoted(t.studyplan_head.plan)),
                (answer(1, &home.using_betula), quoted(t.catalog.fits)),
                (answer(2, &home.using_betula), quoted(t.catalog.saved_chip)),
                (answer(3, &home.using_betula), quoted(sync.pair)),
                (answer(3, &home.using_betula), quoted(t.profile.title)),
                (answer(3, &home.using_betula), quoted(t.app.my_studies)),
                (answer(3, &home.using_betula), quoted(t.studyplan_share.copy_link)),
                (answer(0, &home.for_studies), quoted(t.program.in_catalog)),
                (answer(0, &home.for_studies), quoted(t.myprogram.mine)),
                (answer(3, &home.for_studies), quoted(ProgramTab::Areas.label(*locale))),
                (answer(3, &home.for_studies), quoted(t.catalog.area)),
                (answer(4, &home.for_studies), quoted(t.catalog.confirmed)),
                (answer(7, &home.about_betula), quoted(t.myprogram.mine)),
            ];
            for (text, label) in pairs {
                assert!(text.contains(&label), "{locale:?}: {label} is not in: {text}");
            }
        }
    }

    /// What a chapter quotes of the app (a button, a filter, a label) is what the app says there,
    /// in every language: renamed there, it has to be renamed here.
    #[test]
    fn the_chapters_quote_the_app_as_it_is() {
        for locale in Locale::ALL {
            let t = i18n::texts(*locale);
            let (open, close) = if *locale == Locale::De { ("„", "“") } else { ("“", "”") };
            let quoted = |label: &str| format!("{open}{label}{close}");
            let d = &t.home_detail;
            let point = |chapter: &folia_home::i18n::home_detail::Chapter, n: usize| chapter.points.get(n).map(|point| point.text).unwrap_or_default();
            let sync = &folia_profile::i18n::texts(*locale).sync;
            let pairs = [
                (point(&d.filters, 3), quoted(t.common.reset)),
                (point(&d.timetable, 3), quoted(t.catalog.fits)),
                (point(&d.account, 0), quoted(sync.pair)),
                (point(&d.account, 0), quoted(t.profile.title)),
                (point(&d.account, 1), quoted(t.studyplan_share.copy_link)),
                (point(&d.data, 2), quoted(t.common.not_stated)),
                (point(&d.data, 2), quoted(t.module.time_open)),
                (d.hints.program, quoted(t.catalog.my_program)),
                (d.hints.dates, quoted(t.catalog.confirmed)),
                (d.hints.dates, quoted(t.catalog.fits)),
            ];
            for (text, label) in pairs {
                assert!(text.contains(&label), "{locale:?}: {label} is not in: {text}");
            }
        }
    }

    /// What the privacy notice quotes of the app (a button, a dialog's choice, a page) is what the
    /// app says there, in every language: renamed there, it has to be renamed in the notice (until
    /// 2026-11-18 it named „Weiteres Gerät verbinden", which had become „Gerät verbinden"). The
    /// ways of sharing stand in it under the names the dialog „Teilen" gives them, and a part it
    /// points to is named by that part's heading.
    #[test]
    fn the_privacy_notice_quotes_the_app_as_it_is() {
        for locale in Locale::ALL {
            let t = i18n::texts(*locale);
            let (open, close) = if *locale == Locale::De { ("„", "“") } else { ("“", "”") };
            let quoted = |label: &str| format!("{open}{label}{close}");
            let legal = &t.legal;
            let home = folia_home::i18n::texts(*locale);
            let sync = &folia_profile::i18n::texts(*locale).sync;
            let dialog = &folia_planner::i18n::texts(*locale).studyplan_share_dialog;
            let list = &folia_bookmarks::i18n::texts(*locale).bookmarks_share;
            let text = |texts: &[&'static str], n: usize| texts.get(n).copied().unwrap_or_default();
            let pairs = [
                (text(legal.summary.points, 0), quoted(t.app.my_studies)),
                (text(legal.summary.points, 0), quoted(t.myprogram.mine)),
                (text(legal.local.profile, 5), quoted(dialog.shared)),
                (text(legal.local.beside, 0), quoted(home.today.next)),
                (text(legal.local.beside, 1), quoted(t.catalog.fits)),
                (text(legal.local.beside, 3), quoted(home.install.later)),
                (text(legal.local.beside, 3), quoted(home.install.never)),
                (text(legal.local.texts, 0), quoted(home.install.take)),
                (text(legal.local.texts, 3), quoted(t.profile.title)),
                (text(legal.sync.data, 4), quoted(legal.share.heading)),
                (legal.sync.pairing, quoted(sync.pair)),
                (legal.sync.pairing, quoted(sync.ask_title)),
                (legal.sync.ending, quoted(sync.stop)),
                (legal.sync.ending, quoted(sync.leave)),
                (legal.sync.ending, quoted(t.profile.title)),
                (legal.share.logs, quoted(legal.sync.heading)),
                (legal.share.ending, quoted(dialog.stop)),
                (legal.share.ending, quoted(dialog.withdraw)),
                (legal.calendar.live, quoted(legal.share.heading)),
                (legal.calendar.disclosure, quoted(t.studyplan_export.google)),
                (legal.calendar.ending, quoted(dialog.stop)),
                (legal.rights.finding, quoted(sync.stop)),
            ];
            for (text, label) in pairs {
                assert!(text.contains(&label), "{locale:?}: {label} is not in: {text}");
            }
            assert_eq!((legal.facts.fixed, legal.facts.live, legal.facts.once), (dialog.copy, dialog.stays, dialog.row_once), "{locale:?}: the Stundenplan's dialog");
            assert_eq!((legal.facts.fixed, legal.facts.live, legal.facts.once), (list.copy, list.stays, list.row_once), "{locale:?}: the Merkliste's dialog");
            assert_eq!((dialog.shared, dialog.stop, dialog.withdraw), (list.shared, list.stop, list.withdraw), "{locale:?}: both dialogs");
        }
    }

    /// What the start page's tour quotes of the other areas (`home_detail::quoted_*`) is what
    /// those areas say, in every language.
    #[test]
    fn the_tour_quotes_the_areas_as_they_are() {
        for locale in Locale::ALL {
            let t = i18n::texts(*locale);
            let q = &t.home_detail;
            assert_eq!(q.quoted_in_catalog, t.program.in_catalog, "{locale:?}");
            assert_eq!(q.quoted_week_a, t.studyplan_head.week_a, "{locale:?}");
            assert_eq!(q.quoted_one_clash_per_week, (t.studyplan_head.clashes_per_week)(1), "{locale:?}");
            assert_eq!(q.quoted_copy_link, t.studyplan_share.copy_link, "{locale:?}");
            assert_eq!((q.quoted_download, q.quoted_apple, q.quoted_google), (t.studyplan_export.download, t.studyplan_export.apple, t.studyplan_export.google), "{locale:?}");
        }
    }
}
