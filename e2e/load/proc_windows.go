//go:build windows

package main

import (
	"syscall"
	"time"
	"unsafe"
)

const (
	processQueryLimitedInformation = 0x1000
	processVMRead                  = 0x0010
)

// processCPU is the CPU time (user + kernel) the process has used so far.
func processCPU(pid int) (time.Duration, error) {
	if pid <= 0 {
		return 0, nil
	}
	h, err := syscall.OpenProcess(processQueryLimitedInformation, false, uint32(pid))
	if err != nil {
		return 0, err
	}
	defer syscall.CloseHandle(h)
	var created, exited, kernel, user syscall.Filetime
	if err := syscall.GetProcessTimes(h, &created, &exited, &kernel, &user); err != nil {
		return 0, err
	}
	ticks := func(f syscall.Filetime) time.Duration {
		return time.Duration((int64(f.HighDateTime)<<32 | int64(f.LowDateTime)) * 100)
	}
	return ticks(kernel) + ticks(user), nil
}

var procGetProcessMemoryInfo = syscall.NewLazyDLL("psapi.dll").NewProc("GetProcessMemoryInfo")

type processMemoryCounters struct {
	cb                         uint32
	pageFaultCount             uint32
	peakWorkingSetSize         uintptr
	workingSetSize             uintptr
	quotaPeakPagedPoolUsage    uintptr
	quotaPagedPoolUsage        uintptr
	quotaPeakNonPagedPoolUsage uintptr
	quotaNonPagedPoolUsage     uintptr
	pagefileUsage              uintptr
	peakPagefileUsage          uintptr
}

// processMemory is the working set and its peak, in MB.
func processMemory(pid int) (float64, float64) {
	if pid <= 0 {
		return 0, 0
	}
	h, err := syscall.OpenProcess(processQueryLimitedInformation|processVMRead, false, uint32(pid))
	if err != nil {
		return 0, 0
	}
	defer syscall.CloseHandle(h)
	var c processMemoryCounters
	c.cb = uint32(unsafe.Sizeof(c))
	r, _, _ := procGetProcessMemoryInfo.Call(uintptr(h), uintptr(unsafe.Pointer(&c)), uintptr(c.cb))
	if r == 0 {
		return 0, 0
	}
	return float64(c.workingSetSize) / 1e6, float64(c.peakWorkingSetSize) / 1e6
}
