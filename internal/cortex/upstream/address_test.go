package upstream

import (
	"context"
	"errors"
	"io"
	"net/http"
	"net/netip"
	"net/url"
	"testing"
	"time"
)

func TestOnlyGloballyReachableAddressesArePublic(t *testing.T) {
	for _, tc := range []struct {
		addr   string
		public bool
	}{
		{"8.8.8.8", true},
		{"93.184.215.14", true},
		{"2606:4700:4700::1111", true},
		{"2a00:1450:4001:82a::200e", true},
		{"64:ff9b::808:808", true},  // NAT64 of 8.8.8.8
		{"2002:808:808::1", true},   // 6to4 of 8.8.8.8
		{"::ffff:8.8.8.8", true},    // mapped
		{"127.0.0.1", false},        // loopback
		{"127.255.255.254", false},  // loopback
		{"::1", false},              // loopback
		{"10.0.0.1", false},         // RFC 1918
		{"172.16.0.1", false},       // RFC 1918
		{"172.31.255.255", false},   // RFC 1918
		{"192.168.1.1", false},      // RFC 1918
		{"100.64.0.1", false},       // carrier-grade NAT
		{"100.127.255.255", false},  // carrier-grade NAT
		{"169.254.169.254", false},  // link-local, cloud metadata
		{"fe80::1", false},          // link-local
		{"fe80::1%eth0", false},     // link-local with a zone
		{"fc00::1", false},          // unique local
		{"fd12:3456::1", false},     // unique local
		{"224.0.0.1", false},        // multicast
		{"239.255.255.250", false},  // multicast
		{"ff02::1", false},          // multicast
		{"0.0.0.0", false},          // unspecified
		{"::", false},               // unspecified
		{"255.255.255.255", false},  // broadcast
		{"192.0.2.1", false},        // documentation
		{"198.18.0.1", false},       // benchmarking
		{"::ffff:127.0.0.1", false}, // mapped loopback
		{"::ffff:10.0.0.1", false},  // mapped private
		{"::ffff:169.254.169.254", false},
		{"::127.0.0.1", false},       // IPv4-compatible
		{"64:ff9b::a00:1", false},    // NAT64 of 10.0.0.1
		{"64:ff9b::7f00:1", false},   // NAT64 of 127.0.0.1
		{"2002:a00:1::1", false},     // 6to4 of 10.0.0.1
		{"2002:a9fe:a9fe::1", false}, // 6to4 of 169.254.169.254
		{"2001:db8::1", false},       // documentation
	} {
		ip := netip.MustParseAddr(tc.addr)
		if got := publicAddr(ip); got != tc.public {
			t.Errorf("publicAddr(%s) = %v, want %v", tc.addr, got, tc.public)
		}
		if err := checkAddr(ip); (err == nil) != tc.public || (err != nil && !errors.Is(err, ErrAddressNotAllowed)) {
			t.Errorf("checkAddr(%s) = %v", tc.addr, err)
		}
	}
}

func TestTheDialerRefusesPrivateAddresses(t *testing.T) {
	f := newFakeHost(t, func(w http.ResponseWriter, r *http.Request) { _, _ = io.WriteString(w, "internal") })
	u := openUpstream(t, Options{Policy: testPolicy(nil)}) // AllowPrivate false
	before := scrape(t)

	for _, target := range []string{
		f.srv.URL + "/admin",
		"http://[::1]:9/",
		"http://10.1.2.3/",
		"https://10.1.2.3/",
		"http://169.254.169.254/latest/meta-data/",
		"http://100.64.0.1/",
		"http://192.168.0.1:8080/",
		"http://0.0.0.0:9/",
		"http://[::ffff:127.0.0.1]:9/",
	} {
		start := time.Now()
		var sink memSink
		_, err := u.Fetch(context.Background(), Request{URL: target, Sink: sink.sink})
		if !errors.Is(err, ErrAddressNotAllowed) {
			t.Errorf("%s: err %v, want ErrAddressNotAllowed", target, err)
		}
		if took := time.Since(start); took > 2*time.Second {
			t.Errorf("%s: refused after %s, want before connecting", target, took)
		}
		if sink.calls != 0 {
			t.Errorf("%s: the sink was called", target)
		}
	}
	if f.count() != 0 {
		t.Errorf("the server on 127.0.0.1 got %d requests, want none", f.count())
	}
	for _, hs := range u.HostStates() {
		if hs.FailuresInRow != 0 {
			t.Errorf("a refusal counted as a failure of the host: %+v", hs)
		}
	}
	after := scrape(t)
	if got := after[`cortex_upstream_requests_total{host="other",code="error"}`] - before[`cortex_upstream_requests_total{host="other",code="error"}`]; got != 0 {
		t.Errorf("%v refused requests counted as sent upstream", got)
	}
}

// fakeProxy is an HTTP proxy for the tests: what it is asked for, it answers itself.
func fakeProxy(t *testing.T, handle http.HandlerFunc) (*fakeHost, func(*http.Request) (*url.URL, error)) {
	t.Helper()
	p := newFakeHost(t, handle)
	proxyURL, err := url.Parse(p.srv.URL)
	if err != nil {
		t.Fatal(err)
	}
	return p, func(*http.Request) (*url.URL, error) { return proxyURL, nil }
}

func TestThroughAProxyTheTargetIsCheckedBeforeTheRequest(t *testing.T) {
	proxy, via := fakeProxy(t, func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/to-metadata":
			http.Redirect(w, r, "http://169.254.169.254/latest/meta-data/", http.StatusFound)
		case "/to-loopback":
			http.Redirect(w, r, "http://[::ffff:127.0.0.1]:8100/status", http.StatusFound)
		default:
			_, _ = io.WriteString(w, "via proxy: "+r.URL.String())
		}
	})
	// The proxy itself is on 127.0.0.1: the operator's choice, not checked.
	u := openUpstream(t, Options{Policy: testPolicy(nil), proxy: via})

	var sink memSink
	res, err := u.Fetch(context.Background(), Request{URL: "http://93.184.215.14/page?x=1", Sink: sink.sink})
	if err != nil || res.Status != 200 || string(sink.body) != "via proxy: http://93.184.215.14/page?x=1" {
		t.Fatalf("a public target: got %+v %q (err %v), want it fetched through the proxy", res, sink.body, err)
	}

	for _, target := range []string{
		"http://10.0.0.1/",
		"https://192.168.1.1/",
		"http://[fd00::1]/",
		"http://127.0.0.1:8100/status",
	} {
		if _, err := u.Fetch(context.Background(), Request{URL: target, Sink: discardSink}); !errors.Is(err, ErrAddressNotAllowed) {
			t.Errorf("%s: err %v, want ErrAddressNotAllowed", target, err)
		}
	}
	if proxy.count() != 1 {
		t.Errorf("the proxy got %d requests, want 1", proxy.count())
	}

	// A redirect to a private address is refused too, before it reaches the proxy.
	for _, path := range []string{"/to-metadata", "/to-loopback"} {
		if _, err := u.Fetch(context.Background(), Request{URL: "http://93.184.215.14" + path, Sink: discardSink}); !errors.Is(err, ErrAddressNotAllowed) {
			t.Errorf("%s: err %v, want ErrAddressNotAllowed", path, err)
		}
	}
	if proxy.count() != 3 {
		t.Errorf("the proxy got %d requests, want 3", proxy.count())
	}
}

func TestWithoutAProxyTheProxiedClientNeverConnectsDirectly(t *testing.T) {
	u := openUpstream(t, Options{Policy: testPolicy(nil), AllowPrivate: true})
	req, _ := http.NewRequest(http.MethodGet, "http://93.184.215.14/", nil)
	if _, err := u.proxied.Do(req); err == nil {
		t.Error("the proxied client connected without a proxy")
	}
}
