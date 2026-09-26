//go:build !windows

package main

import (
	"os"
	"strconv"
	"strings"
	"time"
)

// processCPU is the CPU time (user + kernel) the process has used so far (Linux: /proc).
func processCPU(pid int) (time.Duration, error) {
	if pid <= 0 {
		return 0, nil
	}
	b, err := os.ReadFile("/proc/" + strconv.Itoa(pid) + "/stat")
	if err != nil {
		return 0, err
	}
	// The command may contain spaces: the fields start after its closing parenthesis.
	s := string(b)
	fields := strings.Fields(s[strings.LastIndexByte(s, ')')+2:])
	utime, _ := strconv.ParseInt(fields[11], 10, 64)
	stime, _ := strconv.ParseInt(fields[12], 10, 64)
	return time.Duration(utime+stime) * 10 * time.Millisecond, nil // USER_HZ = 100
}

func processMemory(pid int) (float64, float64) {
	if pid <= 0 {
		return 0, 0
	}
	b, err := os.ReadFile("/proc/" + strconv.Itoa(pid) + "/status")
	if err != nil {
		return 0, 0
	}
	var rss, peak float64
	for _, line := range strings.Split(string(b), "\n") {
		f := strings.Fields(line)
		if len(f) < 2 {
			continue
		}
		v, _ := strconv.ParseFloat(f[1], 64)
		switch f[0] {
		case "VmRSS:":
			rss = v / 1000
		case "VmHWM:":
			peak = v / 1000
		}
	}
	return rss, peak
}
