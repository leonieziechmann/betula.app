// Package telemetry holds the metric registry of Cortex. Cortex serves this registry at
// GET /metrics, not metrics.Default: the packages of Radix register their radix_* families in
// metrics.Default when they are imported, and Cortex's endpoint shows only its own.
package telemetry

import "github.com/leonieziechmann/betula/cortex/internal/metrics"

// Registry is where every package of Cortex declares its cortex_* metrics.
var Registry = metrics.NewRegistry()
