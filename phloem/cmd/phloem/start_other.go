//go:build !linux

package main

import (
	"context"
	"fmt"
)

// runStart needs Linux: its firewall is nftables, its drop of privileges Linux's.
func runStart(ctx context.Context, args []string) int {
	fmt.Fprintln(stderr, "Error: phloem start runs on Linux only (nftables); run phloem serve")
	return 2
}

// runProbe needs Linux, as start.
func runProbe(ctx context.Context, args []string) int {
	fmt.Fprintln(stderr, "Error: phloem probe runs on Linux only")
	return 2
}

// dropIfRoot does nothing outside Linux.
func dropIfRoot() error { return nil }
