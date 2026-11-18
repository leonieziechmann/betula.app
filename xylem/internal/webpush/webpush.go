// Package webpush is what Xylem says to a push service for one browser: a message sealed for the
// browser's push subscription (RFC 8291: aes128gcm, one record) and the signature that says the
// request is Xylem's (RFC 8292, VAPID). The standard library alone: ECDH and ECDSA over P-256,
// HKDF-SHA256, AES-128-GCM.
//
// A push service sees who sends (the VAPID key), to which subscription, when and how much; never
// what: only the browser holds the key that opens a message.
package webpush

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/ecdh"
	"crypto/hkdf"
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"fmt"
)

// The sizes of RFC 8291 and RFC 8188.
const (
	// PublicKeyLen is a P-256 public key in the uncompressed form: 0x04, x, y.
	PublicKeyLen = 65
	// AuthLen is a subscription's authentication secret.
	AuthLen = 16

	saltLen    = 16
	recordSize = 4096
	// headerLen: the salt, the record size, the length of the key id, and the sender's public
	// key as the key id.
	headerLen = saltLen + 4 + 1 + PublicKeyLen
	tagLen    = 16

	// MaxPlaintext is what one push message carries: a push service need not take more than
	// 4096 bytes (RFC 8291 section 4), of which the header takes 86, the delimiter of the last
	// record 1 and AES-GCM's tag 16.
	MaxPlaintext = recordSize - headerLen - 1 - tagLen
)

var (
	// ErrTooBig: a plaintext above MaxPlaintext.
	ErrTooBig = errors.New("webpush: the message is larger than a push message carries")
	// ErrSubscription: keys that are no subscription's (a p256dh that is no point of P-256, an
	// auth of another length).
	ErrSubscription = errors.New("webpush: invalid subscription keys")
	// ErrMessage: a body that does not open (Open).
	ErrMessage = errors.New("webpush: the message does not open")
)

// CheckKeys says whether p256dh and auth are the keys of a push subscription: a point of P-256
// in the uncompressed form and 16 bytes.
func CheckKeys(p256dh, auth []byte) error {
	if len(p256dh) != PublicKeyLen || len(auth) != AuthLen {
		return ErrSubscription
	}
	if _, err := ecdh.P256().NewPublicKey(p256dh); err != nil {
		return ErrSubscription
	}
	return nil
}

// Seal seals plaintext for the subscription with the keys p256dh and auth: the body of the
// request to its endpoint, with Content-Encoding: aes128gcm. Every call takes a new key pair and
// a new salt, so two bodies of one plaintext have nothing in common.
func Seal(plaintext, p256dh, auth []byte) ([]byte, error) {
	sender, err := ecdh.P256().GenerateKey(rand.Reader)
	if err != nil {
		return nil, err
	}
	salt := make([]byte, saltLen)
	if _, err := rand.Read(salt); err != nil {
		return nil, err
	}
	return seal(plaintext, p256dh, auth, sender, salt)
}

// seal is Seal with the sender's key pair and the salt given (the example of RFC 8291).
func seal(plaintext, p256dh, auth []byte, sender *ecdh.PrivateKey, salt []byte) ([]byte, error) {
	if len(plaintext) > MaxPlaintext {
		return nil, ErrTooBig
	}
	if len(auth) != AuthLen || len(p256dh) != PublicKeyLen || len(salt) != saltLen {
		return nil, ErrSubscription
	}
	receiver, err := ecdh.P256().NewPublicKey(p256dh)
	if err != nil {
		return nil, ErrSubscription
	}
	secret, err := sender.ECDH(receiver)
	if err != nil {
		return nil, ErrSubscription
	}
	public := sender.PublicKey().Bytes()
	gcm, nonce, err := contentKey(secret, auth, p256dh, public, salt)
	if err != nil {
		return nil, err
	}
	out := make([]byte, 0, headerLen+len(plaintext)+1+tagLen)
	out = append(out, salt...)
	out = binary.BigEndian.AppendUint32(out, recordSize)
	out = append(out, byte(len(public)))
	out = append(out, public...)
	// One record, the last: the plaintext and the delimiter 0x02, no padding.
	record := make([]byte, 0, len(plaintext)+1)
	record = append(append(record, plaintext...), 0x02)
	return gcm.Seal(out, nonce, record, nil), nil
}

// Open opens a body Seal made, as the browser does: with the subscription's private key and its
// authentication secret. Xylem itself never opens anything; its checks do, and whoever tries a
// push service by hand.
func Open(body []byte, receiver *ecdh.PrivateKey, auth []byte) ([]byte, error) {
	if len(body) < headerLen+1+tagLen || len(auth) != AuthLen {
		return nil, ErrMessage
	}
	salt := body[:saltLen]
	size := binary.BigEndian.Uint32(body[saltLen : saltLen+4])
	idLen := int(body[saltLen+4])
	if idLen != PublicKeyLen || uint64(len(body)-headerLen) > uint64(size) {
		return nil, ErrMessage
	}
	public := body[saltLen+5 : headerLen]
	sender, err := ecdh.P256().NewPublicKey(public)
	if err != nil {
		return nil, ErrMessage
	}
	secret, err := receiver.ECDH(sender)
	if err != nil {
		return nil, ErrMessage
	}
	gcm, nonce, err := contentKey(secret, auth, receiver.PublicKey().Bytes(), public, salt)
	if err != nil {
		return nil, err
	}
	record, err := gcm.Open(nil, nonce, body[headerLen:], nil)
	if err != nil {
		return nil, ErrMessage
	}
	// The padding: zeros after the delimiter, which is 0x02 in the last record.
	end := len(record) - 1
	for end >= 0 && record[end] == 0 {
		end--
	}
	if end < 0 || record[end] != 0x02 {
		return nil, ErrMessage
	}
	return record[:end], nil
}

// contentKey derives the content encryption key and the nonce of the first record (RFC 8291
// section 3.4, RFC 8188 section 2.2).
func contentKey(secret, auth, receiverPublic, senderPublic, salt []byte) (cipher.AEAD, []byte, error) {
	keyInfo := "WebPush: info\x00" + string(receiverPublic) + string(senderPublic)
	prkKey, err := hkdf.Extract(sha256.New, secret, auth)
	if err != nil {
		return nil, nil, err
	}
	ikm, err := hkdf.Expand(sha256.New, prkKey, keyInfo, 32)
	if err != nil {
		return nil, nil, err
	}
	prk, err := hkdf.Extract(sha256.New, ikm, salt)
	if err != nil {
		return nil, nil, err
	}
	cek, err := hkdf.Expand(sha256.New, prk, "Content-Encoding: aes128gcm\x00", 16)
	if err != nil {
		return nil, nil, err
	}
	nonce, err := hkdf.Expand(sha256.New, prk, "Content-Encoding: nonce\x00", 12)
	if err != nil {
		return nil, nil, err
	}
	block, err := aes.NewCipher(cek)
	if err != nil {
		return nil, nil, err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, nil, fmt.Errorf("webpush: %w", err)
	}
	return gcm, nonce, nil
}
