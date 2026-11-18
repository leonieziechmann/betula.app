package webpush

import (
	"bytes"
	"crypto/ecdh"
	"crypto/rand"
	"errors"
	"strings"
	"testing"
	"time"
)

func mustDecode(t *testing.T, s string) []byte {
	t.Helper()
	data, err := b64.DecodeString(strings.Join(strings.Fields(s), ""))
	if err != nil {
		t.Fatalf("decode %q: %v", s, err)
	}
	return data
}

// The example of RFC 8291 (section 5 and appendix A): with its keys and its salt, the body is
// the one the RFC prints, octet for octet.
func TestSealIsTheExampleOfRFC8291(t *testing.T) {
	plaintext := mustDecode(t, "V2hlbiBJIGdyb3cgdXAsIEkgd2FudCB0byBiZSBhIHdhdGVybWVsb24")
	if string(plaintext) != "When I grow up, I want to be a watermelon" {
		t.Fatalf("plaintext %q", plaintext)
	}
	receiverPublic := mustDecode(t, "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcx aOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4")
	receiverPrivate := mustDecode(t, "q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94")
	senderPrivate := mustDecode(t, "yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw")
	senderPublic := mustDecode(t, "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIg Dll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8")
	salt := mustDecode(t, "DGv6ra1nlYgDCS1FRnbzlw")
	auth := mustDecode(t, "BTBZMqHH6r4Tts7J_aSIgg")
	want := mustDecode(t, `DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27ml
		mlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPT
		pK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN`)

	sender, err := ecdh.P256().NewPrivateKey(senderPrivate)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(sender.PublicKey().Bytes(), senderPublic) {
		t.Fatal("the sender's public key is not the RFC's")
	}
	got, err := seal(plaintext, receiverPublic, auth, sender, salt)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, want) {
		t.Fatalf("body\n got %x\nwant %x", got, want)
	}
	// 86 octets of header, 41 of plaintext, the delimiter and the tag (the RFC's own
	// "Content-Length: 145" counts one too many: its body, in base64url, is 144 octets).
	if len(got) != 144 {
		t.Fatalf("length %d, want 144", len(got))
	}

	receiver, err := ecdh.P256().NewPrivateKey(receiverPrivate)
	if err != nil {
		t.Fatal(err)
	}
	opened, err := Open(want, receiver, auth)
	if err != nil || !bytes.Equal(opened, plaintext) {
		t.Fatalf("Open = %q, %v", opened, err)
	}
}

func subscription(t *testing.T) (*ecdh.PrivateKey, []byte) {
	t.Helper()
	receiver, err := ecdh.P256().GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	auth := make([]byte, AuthLen)
	if _, err := rand.Read(auth); err != nil {
		t.Fatal(err)
	}
	return receiver, auth
}

func TestSealOpensOnlyForItsSubscription(t *testing.T) {
	receiver, auth := subscription(t)
	message := []byte(`{"web_push":8030,"notification":{"title":"Prüfungsphase","navigate":"https://betula.app/"}}`)
	body, err := Seal(message, receiver.PublicKey().Bytes(), auth)
	if err != nil {
		t.Fatal(err)
	}
	if len(body) != headerLen+len(message)+1+tagLen {
		t.Fatalf("length %d", len(body))
	}
	if got, err := Open(body, receiver, auth); err != nil || !bytes.Equal(got, message) {
		t.Fatalf("Open = %q, %v", got, err)
	}
	again, err := Seal(message, receiver.PublicKey().Bytes(), auth)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Equal(again, body) || bytes.Equal(again[:saltLen], body[:saltLen]) {
		t.Fatal("two bodies of one message share their salt")
	}

	other, otherAuth := subscription(t)
	if _, err := Open(body, other, auth); !errors.Is(err, ErrMessage) {
		t.Fatalf("another key opened it: %v", err)
	}
	if _, err := Open(body, receiver, otherAuth); !errors.Is(err, ErrMessage) {
		t.Fatalf("another secret opened it: %v", err)
	}
	body[len(body)-1] ^= 1
	if _, err := Open(body, receiver, auth); !errors.Is(err, ErrMessage) {
		t.Fatalf("a changed body opened: %v", err)
	}
}

func TestSealBounds(t *testing.T) {
	receiver, auth := subscription(t)
	public := receiver.PublicKey().Bytes()
	largest := bytes.Repeat([]byte("ä"), MaxPlaintext/2)
	largest = append(largest, make([]byte, MaxPlaintext-len(largest))...)
	body, err := Seal(largest, public, auth)
	if err != nil || len(body) != recordSize {
		t.Fatalf("the largest message: %d bytes, %v; want %d", len(body), err, recordSize)
	}
	if _, err := Seal(append(largest, 'x'), public, auth); !errors.Is(err, ErrTooBig) {
		t.Fatalf("one byte more: %v", err)
	}
	if got, err := Seal(nil, public, auth); err != nil || len(got) != headerLen+1+tagLen {
		t.Fatalf("an empty message: %d, %v", len(got), err)
	}

	offCurve := append([]byte{4}, bytes.Repeat([]byte{1}, 64)...)
	for name, keys := range map[string][2][]byte{
		"short p256dh":   {public[:64], auth},
		"compressed":     {append([]byte{2}, public[1:33]...), auth},
		"not on P-256":   {offCurve, auth},
		"short auth":     {public, auth[:15]},
		"long auth":      {public, append(auth[:16:16], 0)},
		"empty":          {nil, nil},
		"point infinity": {make([]byte, 65), auth},
	} {
		if err := CheckKeys(keys[0], keys[1]); !errors.Is(err, ErrSubscription) {
			t.Errorf("%s: CheckKeys = %v", name, err)
		}
		if _, err := Seal([]byte("x"), keys[0], keys[1]); !errors.Is(err, ErrSubscription) {
			t.Errorf("%s: Seal = %v", name, err)
		}
	}
	if err := CheckKeys(public, auth); err != nil {
		t.Fatal(err)
	}
	for _, body := range [][]byte{nil, make([]byte, headerLen), make([]byte, 200)} {
		if _, err := Open(body, receiver, auth); !errors.Is(err, ErrMessage) {
			t.Errorf("Open of %d bytes: %v", len(body), err)
		}
	}
}

func TestKeyAndToken(t *testing.T) {
	key, err := NewKey()
	if err != nil {
		t.Fatal(err)
	}
	if public, err := b64.DecodeString(key.Public()); err != nil || len(public) != PublicKeyLen || public[0] != 4 {
		t.Fatalf("public key %q", key.Public())
	}
	same, err := ParseKey(key.String() + "\n")
	if err != nil || same.Public() != key.Public() || same.String() != key.String() {
		t.Fatalf("ParseKey: %v", err)
	}
	for _, text := range []string{"", "abc", key.String() + "A", strings.Repeat("A", 43)} { // the last: the scalar 0
		if _, err := ParseKey(text); err == nil {
			t.Errorf("ParseKey(%q) took it", text)
		}
	}

	expires := time.Unix(1_800_000_000, 0)
	token, err := key.Token("https://fcm.googleapis.com", "https://betula.app", expires)
	if err != nil {
		t.Fatal(err)
	}
	header := mustDecode(t, strings.Split(token, ".")[0])
	if string(header) != `{"typ":"JWT","alg":"ES256"}` {
		t.Fatalf("header %s", header)
	}
	authorization := key.Authorization(token)
	if !strings.HasPrefix(authorization, "vapid t=") || !strings.HasSuffix(authorization, ", k="+key.Public()) {
		t.Fatalf("authorization %q", authorization)
	}
	claims, public, err := Verify(authorization)
	if err != nil {
		t.Fatal(err)
	}
	if claims.Audience != "https://fcm.googleapis.com" || claims.Subject != "https://betula.app" || !claims.Expires.Equal(expires) {
		t.Fatalf("claims %+v", claims)
	}
	if b64.EncodeToString(public) != key.Public() {
		t.Fatal("the key of the header is not the signer's")
	}

	// A token another key signed, under this key's name.
	other, _ := NewKey()
	forged, _ := other.Token("https://fcm.googleapis.com", "", expires)
	if _, _, err := Verify(key.Authorization(forged)); err == nil {
		t.Fatal("a token of another key was taken")
	}
	// A claim changed after the signature.
	parts := strings.Split(token, ".")
	parts[1] = b64.EncodeToString([]byte(`{"aud":"https://example.org","exp":1800000000}`))
	if _, _, err := Verify(key.Authorization(strings.Join(parts, "."))); err == nil {
		t.Fatal("a changed token was taken")
	}
	for _, header := range []string{"", "Bearer x", "vapid t=a.b.c", "vapid t=" + token, "vapid k=" + key.Public()} {
		if _, _, err := Verify(header); err == nil {
			t.Errorf("Verify(%q) took it", header)
		}
	}
}
