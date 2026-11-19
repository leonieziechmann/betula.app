package server

import (
	"context"
	"crypto/tls"
	"errors"
	"fmt"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"regexp"
	"strings"
	"syscall"
	"time"
)

// Service is a push service: the company a browser hands its messages to. Xylem writes to the
// endpoints of these alone, since an endpoint is an address a stranger gives it, and Xylem
// sends a request there.
type Service struct {
	// Name is the label of its numbers.
	Name string
	// Hosts are where its endpoints are: a name exactly, or with a leading dot every host
	// under it.
	Hosts []string
}

// Services are the push services of the browsers there are (docs/xylem/xylem.md, „The push
// services"): Chrome and what is built from it on every system but Apple's and Edge on Windows
// (Google's FCM), Firefox (Mozilla's autopush), Safari and every web app on an iPhone or iPad
// (Apple), Edge on Windows (Microsoft's WNS).
var Services = []Service{
	{Name: "google", Hosts: []string{"fcm.googleapis.com", "jmt17.google.com"}},
	{Name: "mozilla", Hosts: []string{".push.services.mozilla.com"}},
	{Name: "apple", Hosts: []string{".push.apple.com"}},
	{Name: "microsoft", Hosts: []string{".notify.windows.com"}},
}

// otherService is the label of the hosts of Options.Hosts.
const otherService = "other"

var hostName = regexp.MustCompile(`^\.?[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$`)

// withHosts is the services with the hosts of Options.Hosts as one more.
func withHosts(services []Service, hosts []string) ([]Service, error) {
	out := append([]Service(nil), services...)
	var extra []string
	for _, h := range hosts {
		h = strings.ToLower(strings.TrimSpace(h))
		if h == "" {
			continue
		}
		if len(h) > 253 || !hostName.MatchString(h) {
			return nil, fmt.Errorf("invalid push host %q: a host name, or .name for every host under it", h)
		}
		extra = append(extra, h)
	}
	if len(extra) > 0 {
		out = append(out, Service{Name: otherService, Hosts: extra})
	}
	return out, nil
}

// errEndpoint: an address that is no endpoint of a push service Xylem writes to.
var errEndpoint = errors.New("no endpoint of a known push service")

// endpoint is a subscription's endpoint as Xylem takes it.
type endpoint struct {
	service string // the push service's name
	origin  string // scheme://host[:port]: the audience of the VAPID token
	host    string
}

// endpointOf checks an endpoint: an https address without a user, on the standard port, whose
// host is a push service's. (With Options.Insecure, for the checks: http and any port too.)
func (s *Server) endpointOf(raw string) (endpoint, error) {
	if len(raw) == 0 || len(raw) > s.lim.MaxEndpoint || strings.ContainsAny(raw, " \t\r\n\\") {
		return endpoint{}, errEndpoint
	}
	u, err := url.Parse(raw)
	if err != nil || u.User != nil || u.Fragment != "" || u.Opaque != "" || u.Hostname() == "" {
		return endpoint{}, errEndpoint
	}
	host := strings.ToLower(u.Hostname())
	switch {
	case u.Scheme == "https" && (u.Port() == "" || u.Port() == "443"):
	case s.opt.Insecure && (u.Scheme == "http" || u.Scheme == "https"):
	default:
		return endpoint{host: host}, errEndpoint
	}
	// The origin as a push service names its own: without the port where it is the scheme's.
	origin := u.Scheme + "://" + host
	if port := u.Port(); port != "" && !(u.Scheme == "https" && port == "443") && !(u.Scheme == "http" && port == "80") {
		origin += ":" + port
	}
	for _, svc := range s.services {
		for _, pattern := range svc.Hosts {
			if host == pattern || (strings.HasPrefix(pattern, ".") && strings.HasSuffix(host, pattern)) {
				return endpoint{service: svc.Name, origin: origin, host: host}, nil
			}
		}
	}
	return endpoint{host: host}, errEndpoint
}

// maxUnknown bounds the hosts remembered as refused.
const maxUnknown = 32

// refused remembers the host of an endpoint that was refused, for `xylem status`: a browser
// whose push service Xylem does not know cannot subscribe, and its host is how one learns of
// it. A host is no secret and names nobody; the endpoint itself is never kept or logged.
func (s *Server) refused(host string) {
	if host == "" || len(host) > 253 || !hostName.MatchString(host) {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := s.unknown[host]; ok || len(s.unknown) < maxUnknown {
		s.unknown[host]++
	}
}

// special are the IPv4 ranges that are no address on the internet (RFC 6890 and its updates),
// as Phloem's.
var special = []netip.Prefix{
	netip.MustParsePrefix("0.0.0.0/8"),       // this network
	netip.MustParsePrefix("10.0.0.0/8"),      // private
	netip.MustParsePrefix("100.64.0.0/10"),   // shared address space (CGNAT)
	netip.MustParsePrefix("127.0.0.0/8"),     // loopback
	netip.MustParsePrefix("169.254.0.0/16"),  // link local, the metadata services
	netip.MustParsePrefix("172.16.0.0/12"),   // private
	netip.MustParsePrefix("192.0.0.0/24"),    // IETF protocol assignments
	netip.MustParsePrefix("192.0.2.0/24"),    // documentation
	netip.MustParsePrefix("192.31.196.0/24"), // AS112
	netip.MustParsePrefix("192.52.193.0/24"), // AMT
	netip.MustParsePrefix("192.88.99.0/24"),  // 6to4 relay anycast
	netip.MustParsePrefix("192.168.0.0/16"),  // private
	netip.MustParsePrefix("192.175.48.0/24"), // AS112
	netip.MustParsePrefix("198.18.0.0/15"),   // benchmarking
	netip.MustParsePrefix("198.51.100.0/24"), // documentation
	netip.MustParsePrefix("203.0.113.0/24"),  // documentation
	netip.MustParsePrefix("224.0.0.0/4"),     // multicast
	netip.MustParsePrefix("240.0.0.0/4"),     // reserved, and the broadcast address
}

// public says whether ip is an IPv4 address on the internet.
func public(ip netip.Addr) bool {
	ip = ip.Unmap()
	if !ip.Is4() {
		return false
	}
	for _, p := range special {
		if p.Contains(ip) {
			return false
		}
	}
	return true
}

// publicOnly is the dialer's last word on every connection Xylem opens to a push service,
// after the name was looked up: a public IPv4 address on port 443, whatever the name resolved
// to. A host name of a push service that suddenly leads into the house reaches nothing.
func publicOnly(network, address string, _ syscall.RawConn) error {
	ap, err := netip.ParseAddrPort(address)
	if err != nil {
		return fmt.Errorf("xylem: no address: %s", address)
	}
	if ap.Port() != 443 || !public(ap.Addr()) {
		return fmt.Errorf("xylem: %s is no push service's address", address)
	}
	return nil
}

// pushClient is the client of the requests to the push services: public addresses on 443 alone
// (publicOnly), through no proxy, TLS 1.2 or newer, no redirect followed, every request within
// timeout.
func pushClient(timeout time.Duration, conns int) *http.Client {
	dialer := &net.Dialer{Timeout: 10 * time.Second, KeepAlive: 30 * time.Second, Control: publicOnly}
	return &http.Client{
		Timeout: timeout,
		Transport: &http.Transport{
			Proxy: nil,
			// IPv4 alone: publicOnly takes nothing else, and so no AAAA record is even asked for.
			DialContext: func(ctx context.Context, network, address string) (net.Conn, error) {
				return dialer.DialContext(ctx, "tcp4", address)
			},
			ForceAttemptHTTP2:     true,
			MaxConnsPerHost:       conns,
			MaxIdleConnsPerHost:   conns,
			IdleConnTimeout:       90 * time.Second,
			TLSHandshakeTimeout:   10 * time.Second,
			ResponseHeaderTimeout: timeout,
			TLSClientConfig:       &tls.Config{MinVersion: tls.VersionTLS12},
		},
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}
}
