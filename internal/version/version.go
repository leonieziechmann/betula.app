// Package version names the release of Radix.
//
// Every snapshot carries it (meta key radix_version), so Folia can say which Radix built the data
// it shows. The owner set Radix and Folia to the same version on 2026-09-21 (alpha-0.2.0); since
// 2026-09-23 they differ (Radix alpha-0.3.0, Folia alpha-0.2.2); since 2026-09-27 they name no
// stage (Radix 0.5.0, Folia 1.0.1). Folia's lives in app/src/lib.rs (VERSION).
package version

// Radix is the version of this collector.
const Radix = "0.5.0"
