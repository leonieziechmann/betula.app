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
            let pairs = [
                (answer(0, &home.using_betula), quoted(t.home_detail.heading)),
                (answer(1, &home.using_betula), quoted(t.studyplan_head.plan)),
                (answer(1, &home.using_betula), quoted(t.catalog.fits)),
                (answer(2, &home.using_betula), quoted(t.catalog.saved_chip)),
                (answer(3, &home.using_betula), quoted(t.bookmarks.transfer)),
                (answer(3, &home.using_betula), quoted(t.studyplan_share.copy_link)),
                (answer(0, &home.for_studies), quoted(t.program.in_catalog)),
                (answer(0, &home.for_studies), quoted(t.myprogram.mine)),
                (answer(3, &home.for_studies), quoted(ProgramTab::Areas.label(*locale))),
                (answer(3, &home.for_studies), quoted(t.catalog.area)),
                (answer(4, &home.for_studies), quoted(t.catalog.confirmed)),
                (answer(7, &home.about_betula), quoted(t.myprogram.mine)),
                (home.step_program.text, quoted(t.myprogram.mine)),
            ];
            for (text, label) in pairs {
                assert!(text.contains(&label), "{locale:?}: {label} is not in: {text}");
            }
            // The first step is the button of the first panel, under the same name.
            assert_eq!(home.step_program.title, home.choose_program, "{locale:?}");
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
            let pairs = [
                (point(&d.filters, 3), quoted(t.common.reset)),
                (point(&d.timetable, 3), quoted(t.catalog.fits)),
                (point(&d.account, 0), quoted(t.bookmarks.transfer)),
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
