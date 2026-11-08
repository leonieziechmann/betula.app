package server

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"errors"
	"slices"
	"strconv"

	"github.com/leonieziechmann/betula/phloem/internal/store"
)

// enc is the encoding of every byte value in a frame, a header or a body: base64url without
// padding, decoded strictly (one text for one value).
var enc = base64.RawURLEncoding.Strict()

// b64len is the length of n bytes in enc.
func b64len(n int) int { return enc.EncodedLen(n) }

var errNotB64 = errors.New("not base64url without padding")

// b64 is a byte value of a frame or a body, decoded straight from the JSON text (no copy of the
// text, which for a snapshot is 8 MiB). null and a missing field leave it nil; "" is empty.
type b64 []byte

// UnmarshalJSON implements json.Unmarshaler.
func (b *b64) UnmarshalJSON(raw []byte) error {
	if string(raw) == "null" {
		*b = nil
		return nil
	}
	if len(raw) < 2 || raw[0] != '"' || raw[len(raw)-1] != '"' {
		return errNotB64
	}
	text := raw[1 : len(raw)-1]
	if bytes.IndexByte(text, '\\') >= 0 { // base64url needs no escapes
		return errNotB64
	}
	out := make([]byte, enc.DecodedLen(len(text)))
	n, err := enc.Decode(out, text)
	if err != nil {
		return errNotB64
	}
	*b = out[:n]
	return nil
}

// MarshalJSON implements json.Marshaler.
func (b b64) MarshalJSON() ([]byte, error) { return appendB64(nil, b), nil }

// decode64 decodes a header value of want bytes.
func decode64(text string, want int) ([]byte, bool) {
	out, err := enc.DecodeString(text)
	if err != nil || len(out) != want {
		return nil, false
	}
	return out, true
}

// inFrame is every frame a device sends, by its "t" (docs/phloem/protocol.md section 4).
type inFrame struct {
	T      string `json:"t"`
	Space  b64    `json:"space"`
	Token  b64    `json:"token"`
	Device b64    `json:"device"`
	Since  int64  `json:"since"`
	ID     string `json:"id"`
	Data   b64    `json:"data"`
	Upto   *int64 `json:"upto"`
	Parts  []b64  `json:"parts"`
	To     *int   `json:"to"` // signal
}

// The frames Phloem sends that never change.
var (
	compactFrame = []byte(`{"t":"compact"}`)
	goneFrame    = []byte(`{"t":"gone"}`)
)

func appendB64(b, data []byte) []byte {
	n := enc.EncodedLen(len(data))
	b = slices.Grow(b, n+2)
	b = append(b, '"')
	start := len(b)
	b = b[:start+n]
	enc.Encode(b[start:], data)
	return append(b, '"')
}

func appendSlots(b []byte, slots []int) []byte {
	b = append(b, '[')
	for i, slot := range slots {
		if i > 0 {
			b = append(b, ',')
		}
		b = strconv.AppendInt(b, int64(slot), 10)
	}
	return append(b, ']')
}

// appendEntry writes the fields of an entry and the closing brace.
func appendEntry(b []byte, nr int64, slot int, data []byte) []byte {
	b = append(b, `"nr":`...)
	b = strconv.AppendInt(b, nr, 10)
	b = append(b, `,"slot":`...)
	b = strconv.AppendInt(b, int64(slot), 10)
	b = append(b, `,"data":`...)
	b = appendB64(b, data)
	return append(b, '}')
}

// welcomeFrame answers a valid hello, in one allocation of the size it needs.
func welcomeFrame(slot int, head, now int64, devices []int, snap *store.Snapshot, entries []store.Entry) []byte {
	size := 128 + 4*len(devices)
	if snap != nil {
		size += b64len(len(snap.Data)) + 48
	}
	for _, e := range entries {
		size += b64len(len(e.Data)) + 64
	}
	b := make([]byte, 0, size)
	b = append(b, `{"t":"welcome","slot":`...)
	b = strconv.AppendInt(b, int64(slot), 10)
	b = append(b, `,"head":`...)
	b = strconv.AppendInt(b, head, 10)
	b = append(b, `,"now":`...)
	b = strconv.AppendInt(b, now, 10)
	b = append(b, `,"devices":`...)
	b = appendSlots(b, devices)
	b = append(b, `,"snapshot":`...)
	if snap == nil {
		b = append(b, "null"...)
	} else {
		b = append(b, `{"upto":`...)
		b = strconv.AppendInt(b, snap.Upto, 10)
		b = append(b, `,"data":`...)
		b = appendB64(b, snap.Data)
		b = append(b, '}')
	}
	b = append(b, `,"entries":[`...)
	for i, e := range entries {
		if i > 0 {
			b = append(b, ',')
		}
		b = append(b, '{')
		b = appendEntry(b, e.Nr, e.Slot, e.Data)
	}
	return append(b, "]}"...)
}

// entryFrame is another device's push; one frame for every device it goes to.
func entryFrame(nr int64, slot int, data []byte) []byte {
	b := make([]byte, 0, b64len(len(data))+64)
	b = append(b, `{"t":"entry",`...)
	return appendEntry(b, nr, slot, data)
}

func storedFrame(id string, nr int64) []byte {
	b, _ := json.Marshal(struct {
		T  string `json:"t"`
		ID string `json:"id"`
		Nr int64  `json:"nr"`
	}{"stored", id, nr})
	return b
}

// errorFrame is an error, of the push id when it has one.
func errorFrame(code, id string) []byte {
	b, _ := json.Marshal(struct {
		T    string `json:"t"`
		Code string `json:"code"`
		ID   string `json:"id,omitempty"`
	}{"error", code, id})
	return b
}

func snapshottedFrame(upto int64) []byte {
	b := append([]byte(`{"t":"snapshotted","upto":`), strconv.FormatInt(upto, 10)...)
	return append(b, '}')
}

func presenceFrame(slots []int) []byte {
	b := append([]byte(`{"t":"presence","devices":`), appendSlots(nil, slots)...)
	return append(b, '}')
}

// partFrame tells the sockets that watch a part its new version.
func partFrame(id []byte, rev int64) []byte {
	b := appendB64([]byte(`{"t":"part","id":`), id)
	b = append(b, `,"rev":`...)
	b = strconv.AppendInt(b, rev, 10)
	return append(b, '}')
}

// partGoneFrame tells the sockets that watch a part that it is gone.
func partGoneFrame(id []byte) []byte {
	b := appendB64([]byte(`{"t":"part","id":`), id)
	return append(b, `,"gone":true}`...)
}

// watchingFrame answers a watch: the version of every part now, in the order asked (0 for one
// Phloem does not have, revGone for one that is gone).
func watchingFrame(ids [][]byte, revs []int64) []byte {
	b := make([]byte, 0, 32+len(ids)*(b64len(partLen)+24))
	b = append(b, `{"t":"watching","parts":[`...)
	for i, id := range ids {
		if i > 0 {
			b = append(b, ',')
		}
		b = appendB64(append(b, `{"id":`...), id)
		if revs[i] == revGone {
			b = append(b, `,"gone":true}`...)
			continue
		}
		b = append(b, `,"rev":`...)
		b = strconv.AppendInt(b, revs[i], 10)
		b = append(b, '}')
	}
	return append(b, "]}"...)
}

// pairFrame tells the devices of a space what became of one of its parcels; state is one of
// the store's, which need no escaping.
func pairFrame(id []byte, state string) []byte {
	b := appendB64([]byte(`{"t":"pair","id":`), id)
	b = append(b, `,"state":"`...)
	b = append(b, state...)
	return append(b, `"}`...)
}

// signalFrame hands a device's signal to the device it is for, named by the slot it came from.
func signalFrame(from int, data []byte) []byte {
	b := make([]byte, 0, 40+b64len(len(data)))
	b = append(b, `{"t":"signal","from":`...)
	b = strconv.AppendInt(b, int64(from), 10)
	b = append(b, `,"data":`...)
	b = appendB64(b, data)
	return append(b, '}')
}

// turnFrame answers turn: the ICE servers and the socket's credential; no URLs without TURN.
func turnFrame(urls []string, user, password string) []byte {
	if len(urls) == 0 {
		return []byte(`{"t":"turn","urls":[]}`)
	}
	data, _ := json.Marshal(struct {
		T        string   `json:"t"`
		URLs     []string `json:"urls"`
		User     string   `json:"user"`
		Password string   `json:"password"`
	}{"turn", urls, user, password})
	return data
}
