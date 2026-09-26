package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strings"
)

// logstats reads Folia's JSON log (event http.request) and says what each kind of page cost the
// server: its own measure from the request's arrival to the answer, cache hits and misses apart.
func logstats(path string, since string) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close()
	type key struct{ kind, cache string }
	ms := map[key][]float64{}
	bytes := map[key]float64{}
	s := bufio.NewScanner(f)
	s.Buffer(make([]byte, 1<<20), 1<<20)
	for s.Scan() {
		var line struct {
			Timestamp string  `json:"timestamp"`
			Event     string  `json:"event"`
			Path      string  `json:"path"`
			Status    int     `json:"status"`
			Ms        float64 `json:"ms"`
			Cache     string  `json:"cache"`
			Bytes     float64 `json:"bytes"`
		}
		if json.Unmarshal(s.Bytes(), &line) != nil || line.Event != "http.request" {
			continue
		}
		if since != "" && line.Timestamp < since {
			continue
		}
		kind := kindOf(line.Path)
		if kind == "other" {
			kind = labelOf(line.Path)
			if strings.HasPrefix(line.Path, "/calendar/") {
				kind = "ics"
			}
			if line.Path == "/livez" {
				kind = "livez"
			}
		}
		k := key{kind, line.Cache}
		ms[k] = append(ms[k], line.Ms)
		bytes[k] += line.Bytes
	}
	keys := make([]key, 0, len(ms))
	for k := range ms {
		keys = append(keys, k)
	}
	sort.Slice(keys, func(i, j int) bool {
		if keys[i].kind != keys[j].kind {
			return keys[i].kind < keys[j].kind
		}
		return keys[i].cache < keys[j].cache
	})
	fmt.Printf("%-22s %-5s %7s %8s %8s %8s %8s %8s\n", "kind", "cache", "n", "mean", "p50", "p90", "p99", "max ms")
	for _, k := range keys {
		v := ms[k]
		sort.Float64s(v)
		var sum float64
		for _, x := range v {
			sum += x
		}
		fmt.Printf("%-22s %-5s %7d %8.2f %8.2f %8.2f %8.2f %8.2f\n", k.kind, k.cache, len(v), sum/float64(len(v)), pct(v, .5), pct(v, .9), pct(v, .99), pct(v, 1))
	}
	return s.Err()
}
