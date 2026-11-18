package webpush

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"strings"
	"time"
)

// Key is Xylem's VAPID key (RFC 8292): a key pair of P-256. Its public half is what a browser
// subscribes with (applicationServerKey), and a push service takes a message for that
// subscription only with a token this key signed. So the key is as old as the first
// subscription: with another key, none of them can be written to any more.
type Key struct {
	private *ecdsa.PrivateKey
	public  []byte // the uncompressed form, 65 bytes
}

var b64 = base64.RawURLEncoding

// NewKey makes a key.
func NewKey() (*Key, error) {
	private, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return nil, err
	}
	return newKey(private)
}

// ParseKey reads a key as String wrote it.
func ParseKey(text string) (*Key, error) {
	scalar, err := b64.DecodeString(strings.TrimSpace(text))
	if err != nil || len(scalar) != 32 {
		return nil, errors.New("webpush: a VAPID key is 32 bytes in base64url")
	}
	private, err := ecdsa.ParseRawPrivateKey(elliptic.P256(), scalar)
	if err != nil {
		return nil, fmt.Errorf("webpush: invalid VAPID key: %w", err)
	}
	return newKey(private)
}

func newKey(private *ecdsa.PrivateKey) (*Key, error) {
	public, err := private.PublicKey.Bytes()
	if err != nil {
		return nil, err
	}
	return &Key{private: private, public: public}, nil
}

// String is the private key, for the place it is kept: its scalar in base64url.
func (k *Key) String() string {
	scalar, err := k.private.Bytes()
	if err != nil {
		return ""
	}
	return b64.EncodeToString(scalar)
}

// Public is the public key in base64url: what a browser subscribes with.
func (k *Key) Public() string { return b64.EncodeToString(k.public) }

// MaxTokenLife is the longest a token may be valid from the moment a push service reads it
// (RFC 8292 section 2).
const MaxTokenLife = 24 * time.Hour

// Token signs a token for the push service of origin audience (scheme://host[:port] of the
// subscription's endpoint), valid until expires. subject is where the service's operators reach
// Xylem's: a mailto: or https: address.
func (k *Key) Token(audience, subject string, expires time.Time) (string, error) {
	claims, err := json.Marshal(struct {
		Aud string `json:"aud"`
		Exp int64  `json:"exp"`
		Sub string `json:"sub,omitempty"`
	}{audience, expires.Unix(), subject})
	if err != nil {
		return "", err
	}
	signed := b64.EncodeToString([]byte(`{"typ":"JWT","alg":"ES256"}`)) + "." + b64.EncodeToString(claims)
	digest := sha256.Sum256([]byte(signed))
	r, s, err := ecdsa.Sign(rand.Reader, k.private, digest[:])
	if err != nil {
		return "", err
	}
	// ES256 in a JWS: r and s as 32 bytes each, not ASN.1 (RFC 7518 section 3.4).
	signature := make([]byte, 64)
	r.FillBytes(signature[:32])
	s.FillBytes(signature[32:])
	return signed + "." + b64.EncodeToString(signature), nil
}

// Authorization is the Authorization header of a request with token.
func (k *Key) Authorization(token string) string {
	return "vapid t=" + token + ", k=" + k.Public()
}

// Claims are what a token says.
type Claims struct {
	Audience string
	Subject  string
	Expires  time.Time
}

// Verify reads the Authorization header of a request as a push service does: the claims of its
// token, if the key it names signed it. The caller checks the audience, the time, and that the
// key is the subscription's.
func Verify(authorization string) (Claims, []byte, error) {
	bad := errors.New("webpush: invalid VAPID authorization")
	rest, ok := strings.CutPrefix(authorization, "vapid ")
	if !ok {
		return Claims{}, nil, bad
	}
	var token, key string
	for _, part := range strings.Split(rest, ",") {
		name, value, _ := strings.Cut(strings.TrimSpace(part), "=")
		switch name {
		case "t":
			token = value
		case "k":
			key = value
		}
	}
	public, err := b64.DecodeString(key)
	if err != nil || len(public) != PublicKeyLen {
		return Claims{}, nil, bad
	}
	verifier, err := ecdsa.ParseUncompressedPublicKey(elliptic.P256(), public)
	if err != nil {
		return Claims{}, nil, bad
	}
	parts := strings.Split(token, ".")
	if len(parts) != 3 {
		return Claims{}, nil, bad
	}
	header, err1 := b64.DecodeString(parts[0])
	body, err2 := b64.DecodeString(parts[1])
	signature, err3 := b64.DecodeString(parts[2])
	if err1 != nil || err2 != nil || err3 != nil || len(signature) != 64 {
		return Claims{}, nil, bad
	}
	var head struct{ Typ, Alg string }
	if json.Unmarshal(header, &head) != nil || head.Alg != "ES256" {
		return Claims{}, nil, bad
	}
	digest := sha256.Sum256([]byte(parts[0] + "." + parts[1]))
	r, s := new(big.Int).SetBytes(signature[:32]), new(big.Int).SetBytes(signature[32:])
	if !ecdsa.Verify(verifier, digest[:], r, s) {
		return Claims{}, nil, bad
	}
	var claims struct {
		Aud string `json:"aud"`
		Exp int64  `json:"exp"`
		Sub string `json:"sub"`
	}
	if json.Unmarshal(body, &claims) != nil {
		return Claims{}, nil, bad
	}
	return Claims{Audience: claims.Aud, Subject: claims.Sub, Expires: time.Unix(claims.Exp, 0)}, public, nil
}
