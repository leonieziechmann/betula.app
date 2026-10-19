// Package version names the release of Radix.
//
// Every snapshot carries it (meta key radix_version), so Folia can say which Radix built the data
// it shows. The owner set Radix and Folia to the same version on 2026-09-21 (alpha-0.2.0); since
// 2026-09-23 they differ (Radix alpha-0.3.0, Folia alpha-0.2.2); since 2026-09-27 they name no
// stage (Radix 0.5.0, Folia 1.0.1); 2026-09-29 Radix 0.6.0, which builds again at start when
// another release built the catalog. Since 2026-10-15 by SemVer, raised with every change to Radix
// (owner: „immer wenn wir changes machen update die version number based on sem ver"; CLAUDE.md,
// „Versions"): 0.7.0 the same day (owner: „Radix bitte auch auf 0.7.0 hochziehen") for what came
// since 0.6.0 unversioned, offline through Cortex (2026-10-04) and a Termin read as a series of
// dates (2026-10-13); 0.7.1 on 2026-10-16, where a prerequisite text rules a module out or a
// module replaces one, the module is no prerequisite (docs/radix/data-sources.md §15); 0.8.0 on
// 2026-10-18, a module of a Wahlpflicht slot of the plan that no source gives a kind is
// Wahlpflicht, inferred (schema 14; docs/radix/schema-v2.md, „Kinds from the plan's slots"); 0.9.0
// on 2026-10-19, whom QIS lists an event for, and the courses of a module held at two campuses with
// the programs that take each (schema 15; docs/radix/schema-v2.md, „Tracks"); 0.9.1 the same day,
// no courses where QIS lists the same programs at each, and where a program is taught by where QIS
// lists its events before its compulsory modules (schema 16, „Better criteria"); 0.10.0 the same
// day, the choices among a module's events — one event more than once for other programs, in
// groups, or contents to choose from — and its extras, read from the SWS, the forms, the titles and
// whom QIS lists each event for (schema 17; docs/radix/schema-v2.md, „Choices"). Folia's lives in
// folia/crates/shell/src/document.rs (VERSION).
package version

// Radix is the version of this collector.
const Radix = "0.10.0"
