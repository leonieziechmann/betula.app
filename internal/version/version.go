// Package version names the release of Radix.
//
// Every snapshot carries it (meta key radix_version), so Folia can say which Radix built the data
// it shows. The owner set Radix and Folia to the same version on 2026-09-21; Folia's lives in
// app/src/lib.rs (VERSION).
package version

// Radix is the version of this collector.
const Radix = "alpha-0.2.0"
