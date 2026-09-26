package main

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"math/rand"
	"os"
	"sort"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

type runOptions struct {
	scenario    string
	rates       []float64 // open model: arrivals per second, one step each
	concs       []int     // closed model: workers that loop without pause, one step each
	step        time.Duration
	drain       time.Duration
	maxInflight int
	pid         int
	probe       bool
	abortP99    time.Duration
	abortErr    float64
	out         string
	name        string
	uniform     bool
}

// LabelStats is one kind of request within a step.
type LabelStats struct {
	N      int            `json:"n"`
	Status map[string]int `json:"status"`
	Errors map[string]int `json:"errors,omitempty"`
	Hit    int            `json:"hit,omitempty"`
	Miss   int            `json:"miss,omitempty"`
	P50    float64        `json:"p50_ms"`
	P90    float64        `json:"p90_ms"`
	P99    float64        `json:"p99_ms"`
	Max    float64        `json:"max_ms"`
	Mean   float64        `json:"mean_ms"`
	TTFB50 float64        `json:"ttfb50_ms"`
	MB     float64        `json:"mb"`
}

// StepStats is one step of a run.
type StepStats struct {
	Run        string                 `json:"run"`
	Scenario   string                 `json:"scenario"`
	Step       int                    `json:"step"`
	Offered    float64                `json:"offered_per_s,omitempty"`
	Workers    int                    `json:"workers,omitempty"`
	Seconds    float64                `json:"seconds"`
	Jobs       int64                  `json:"jobs"`
	Dropped    int64                  `json:"dropped"`
	Requests   int                    `json:"requests"`
	ReqPerS    float64                `json:"req_per_s"`
	OKPerS     float64                `json:"ok_per_s"`
	MBPerS     float64                `json:"mb_per_s"`
	CPUPercent float64                `json:"server_cpu_pct,omitempty"`
	CPUPerReq  float64                `json:"server_cpu_ms_per_req,omitempty"`
	WorkingMB  float64                `json:"server_ws_mb,omitempty"`
	PeakMB     float64                `json:"server_peak_ws_mb,omitempty"`
	Labels     map[string]*LabelStats `json:"labels"`
	ColdWrap   bool                   `json:"cold_list_wrapped,omitempty"`
}

type collector struct {
	mu   sync.Mutex
	byLb map[string][]Result
}

func (c *collector) add(rs []Result) {
	c.mu.Lock()
	for _, r := range rs {
		c.byLb[r.Label] = append(c.byLb[r.Label], r)
	}
	c.mu.Unlock()
}

func pct(sorted []float64, p float64) float64 {
	if len(sorted) == 0 {
		return 0
	}
	i := int(math.Ceil(p*float64(len(sorted)))) - 1
	if i < 0 {
		i = 0
	}
	if i >= len(sorted) {
		i = len(sorted) - 1
	}
	return sorted[i]
}

func summarize(rs []Result) *LabelStats {
	s := &LabelStats{Status: map[string]int{}, Errors: map[string]int{}}
	var lat, ttfb []float64
	var sum float64
	for _, r := range rs {
		s.N++
		if r.Err != "" {
			s.Errors[r.Err]++
		}
		if r.Status != 0 {
			s.Status[fmt.Sprint(r.Status)]++
		}
		switch r.Cache {
		case "hit":
			s.Hit++
		case "miss":
			s.Miss++
		}
		ms := float64(r.Dur.Microseconds()) / 1000
		lat = append(lat, ms)
		ttfb = append(ttfb, float64(r.TTFB.Microseconds())/1000)
		sum += ms
		s.MB += float64(r.Bytes) / 1e6
	}
	sort.Float64s(lat)
	sort.Float64s(ttfb)
	s.P50, s.P90, s.P99, s.Max = pct(lat, .5), pct(lat, .9), pct(lat, .99), pct(lat, 1)
	s.TTFB50 = pct(ttfb, .5)
	if s.N > 0 {
		s.Mean = sum / float64(s.N)
	}
	if len(s.Errors) == 0 {
		s.Errors = nil
	}
	return s
}

func run(e *Env, o runOptions) error {
	var choose func(r *rand.Rand) string
	if strings.Contains(o.scenario, ",") || strings.Contains(o.scenario, "=") {
		f, err := mix(o.scenario)
		if err != nil {
			return err
		}
		choose = f
	} else {
		if _, ok := jobs[o.scenario]; !ok {
			return fmt.Errorf("unknown scenario %q", o.scenario)
		}
		choose = func(*rand.Rand) string { return o.scenario }
	}
	var out *os.File
	if o.out != "" {
		f, err := os.OpenFile(o.out, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
		if err != nil {
			return err
		}
		defer f.Close()
		out = f
	}

	steps := len(o.rates)
	if len(o.concs) > 0 {
		steps = len(o.concs)
	}
	for i := 0; i < steps; i++ {
		col := &collector{byLb: map[string][]Result{}}
		var jobsStarted, dropped atomic.Int64
		var inflight atomic.Int64
		var wg sync.WaitGroup
		ctx, cancel := context.WithCancel(context.Background())
		cpu0, _ := processCPU(o.pid)
		began := time.Now()
		stop := began.Add(o.step)
		rng := rand.New(rand.NewSource(time.Now().UnixNano()))

		launch := func() {
			name := choose(rng)
			jobsStarted.Add(1)
			inflight.Add(1)
			wg.Add(1)
			go func() {
				defer wg.Done()
				defer inflight.Add(-1)
				col.add(jobs[name](context.Background(), e))
			}()
		}

		// The container's healthcheck, once a second, next to the load.
		var probeWG sync.WaitGroup
		if o.probe {
			probeWG.Add(1)
			go func() {
				defer probeWG.Done()
				t := time.NewTicker(time.Second)
				defer t.Stop()
				for {
					select {
					case <-ctx.Done():
						return
					case <-t.C:
						go func() { col.add(jobs["livez"](context.Background(), e)) }()
					}
				}
			}()
		}

		st := StepStats{Run: o.name, Scenario: o.scenario, Step: i + 1}
		if len(o.concs) > 0 {
			st.Workers = o.concs[i]
			var ww sync.WaitGroup
			for w := 0; w < o.concs[i]; w++ {
				ww.Add(1)
				go func(w int) {
					defer ww.Done()
					r := rand.New(rand.NewSource(time.Now().UnixNano() + int64(w)))
					for time.Now().Before(stop) {
						name := choose(r)
						jobsStarted.Add(1)
						col.add(jobs[name](context.Background(), e))
					}
				}(w)
			}
			ww.Wait()
		} else {
			rate := o.rates[i]
			st.Offered = rate
			next := began
			for {
				now := time.Now()
				if !now.Before(stop) {
					break
				}
				// Every arrival that is due starts now: coarse timers must not lower the rate.
				for !next.After(now) && next.Before(stop) {
					if int(inflight.Load()) >= o.maxInflight {
						dropped.Add(1)
					} else {
						launch()
					}
					gap := 1 / rate
					if !o.uniform {
						gap = rng.ExpFloat64() / rate
					}
					next = next.Add(time.Duration(gap * float64(time.Second)))
				}
				sleep := time.Until(next)
				if sleep > 50*time.Millisecond {
					sleep = 50 * time.Millisecond
				}
				if sleep > 0 {
					time.Sleep(sleep)
				}
			}
		}
		issued := time.Since(began)
		// Let what was started finish, so the step's numbers (and the server's CPU) are complete.
		done := make(chan struct{})
		go func() { wg.Wait(); close(done) }()
		select {
		case <-done:
		case <-time.After(o.drain):
			fmt.Fprintf(os.Stderr, "  (step %d: %d jobs still running after %s, counted without them)\n", i+1, inflight.Load(), o.drain)
		}
		cancel()
		probeWG.Wait()
		elapsed := time.Since(began)
		cpu1, _ := processCPU(o.pid)
		ws, peak := processMemory(o.pid)

		col.mu.Lock()
		st.Labels = map[string]*LabelStats{}
		var reqs, ok int
		var mb float64
		for lb, rs := range col.byLb {
			s := summarize(rs)
			st.Labels[lb] = s
			if strings.HasPrefix(lb, "job:") || lb == "livez" {
				continue
			}
			reqs += s.N
			mb += s.MB
			for code, n := range s.Status {
				if code < "400" {
					ok += n
				}
			}
		}
		col.mu.Unlock()
		st.Seconds = issued.Seconds()
		st.Jobs = jobsStarted.Load()
		st.Dropped = dropped.Load()
		st.Requests = reqs
		st.ReqPerS = float64(reqs) / issued.Seconds()
		st.OKPerS = float64(ok) / issued.Seconds()
		st.MBPerS = mb / elapsed.Seconds()
		st.ColdWrap = e.coldWrap.Load()
		if o.pid > 0 && cpu1 > cpu0 {
			st.CPUPercent = 100 * float64(cpu1-cpu0) / float64(elapsed)
			if reqs > 0 {
				st.CPUPerReq = float64((cpu1 - cpu0).Microseconds()) / 1000 / float64(reqs)
			}
			st.WorkingMB, st.PeakMB = ws, peak
		}
		printStep(st)
		if out != nil {
			b, _ := json.Marshal(st)
			out.Write(append(b, '\n'))
		}
		if reason := abortReason(st, o); reason != "" {
			fmt.Printf("  stopping: %s\n", reason)
			return nil
		}
	}
	return nil
}

func abortReason(st StepStats, o runOptions) string {
	var n, bad int
	var worst float64
	for lb, s := range st.Labels {
		if strings.HasPrefix(lb, "job:") || lb == "livez" {
			continue
		}
		n += s.N
		for code, k := range s.Status {
			if code >= "500" || code == "429" {
				bad += k
			}
		}
		for _, k := range s.Errors {
			bad += k
		}
		if s.P99 > worst && s.N >= 20 {
			worst = s.P99
		}
	}
	if n > 0 && o.abortErr > 0 && float64(bad)/float64(n) > o.abortErr {
		return fmt.Sprintf("%.0f %% of the requests failed or were turned away", 100*float64(bad)/float64(n))
	}
	if o.abortP99 > 0 && worst > float64(o.abortP99.Milliseconds()) {
		return fmt.Sprintf("p99 %.0f ms above %s", worst, o.abortP99)
	}
	return ""
}

func printStep(st StepStats) {
	head := fmt.Sprintf("step %d", st.Step)
	if st.Offered > 0 {
		head += fmt.Sprintf("  offered %.1f jobs/s", st.Offered)
	}
	if st.Workers > 0 {
		head += fmt.Sprintf("  %d workers", st.Workers)
	}
	head += fmt.Sprintf("  %.0fs  jobs %d", st.Seconds, st.Jobs)
	if st.Dropped > 0 {
		head += fmt.Sprintf(" (dropped %d)", st.Dropped)
	}
	head += fmt.Sprintf("  req/s %.1f  ok/s %.1f  %.2f MB/s", st.ReqPerS, st.OKPerS, st.MBPerS)
	if st.CPUPercent > 0 {
		head += fmt.Sprintf("  server CPU %.0f %%  %.2f ms/req  WS %.0f MB (peak %.0f)", st.CPUPercent, st.CPUPerReq, st.WorkingMB, st.PeakMB)
	}
	if st.ColdWrap {
		head += "  [cold list wrapped: later pages were cached]"
	}
	fmt.Println(head)
	labels := make([]string, 0, len(st.Labels))
	for lb := range st.Labels {
		labels = append(labels, lb)
	}
	sort.Strings(labels)
	for _, lb := range labels {
		s := st.Labels[lb]
		codes := make([]string, 0, len(s.Status))
		for c, n := range s.Status {
			codes = append(codes, fmt.Sprintf("%s:%d", c, n))
		}
		sort.Strings(codes)
		errs := ""
		for e, n := range s.Errors {
			errs += fmt.Sprintf(" %s:%d", e, n)
		}
		cache := ""
		if s.Hit+s.Miss > 0 {
			cache = fmt.Sprintf(" hit/miss %d/%d", s.Hit, s.Miss)
		}
		fmt.Printf("  %-22s n=%-6d p50 %7.1f  p90 %7.1f  p99 %7.1f  max %7.1f ms  ttfb50 %6.1f  %8.2f MB  %s%s%s\n",
			lb, s.N, s.P50, s.P90, s.P99, s.Max, s.TTFB50, s.MB, strings.Join(codes, " "), cache, errs)
	}
}
