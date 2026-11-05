//! Plan 39 P39e, §2.7: one Web Push message, encrypted for one phone (RFC 8291, `aes128gcm`) and signed for its
//! push service (RFC 8292, VAPID).
//!
//! **Why ring, and nothing new.** ring 0.17 is already compiled into herdr for TLS (rustls under ureq), and it has
//! each primitive the two RFCs use: P-256 ECDH with a fresh key per message (`agreement`), HKDF-SHA-256 (`hkdf`),
//! AES-128-GCM (`aead`) and ECDSA P-256 with the fixed `r || s` signature a JWT's ES256 wants (`signature`). The
//! `web-push` crate would bring its own HTTP client and an OpenSSL or `ece` backend; `p256`, `hkdf` and `aes-gcm`
//! from RustCrypto would be about a dozen packages new to Cargo.lock. With ring the lock gains none, and the whole
//! scheme is the hundred lines below, checked against RFC 8291's own worked example (Appendix A) in the tests.
//!
//! **What leaves the tailnet.** The push service (Apple's, for the owner's iPhone) carries the body to the phone. It
//! sees the ciphertext, its length, the phone's endpoint and the VAPID token (this PC's public key, the push
//! service's own origin, an expiry and `SUBJECT`); only the phone holds the key to read the body. The body itself is
//! kept to what `alerts::payload` writes: the space's name, the agent's name, its state and its pane id.

use std::fmt;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::KeyPair;
use ring::{aead, agreement, hkdf, signature};

/// The record size written in the header. A push is one record (RFC 8291 §4), and this is the size every browser
/// uses; the body here is far smaller.
pub const RECORD_SIZE: u32 = 4096;
/// A push body's plaintext may be at most this long (RFC 8291 §4: 4096 less the 86-octet header, the delimiter and
/// the 16-octet tag).
pub const PLAINTEXT_MOST: usize = 3993;
/// The VAPID token's `sub`, the contact the push service may write to (RFC 8292 §2.1). A placeholder on purpose: the
/// owner's address would leave the tailnet in every token. Apple asks for a `mailto:` or `https:` URI; whether it
/// takes this one is the owner's live check (record p39e).
pub const SUBJECT: &str = "mailto:nirvana-remote@example.com";
/// How long a VAPID token is good for; RFC 8292 §2 allows at most 24 h.
pub const TOKEN_FOR: u64 = 12 * 60 * 60;
const _: () = assert!(
    TOKEN_FOR <= 24 * 60 * 60,
    "RFC 8292 §2: a VAPID token lives 24 h at most"
);

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// An HKDF output length as ring asks for it.
struct Len(usize);

impl hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

fn expand(prk: &hkdf::Prk, info: &[&[u8]], out: &mut [u8]) -> Result<(), String> {
    prk.expand(info, Len(out.len()))
        .and_then(|okm| okm.fill(out))
        .map_err(|_| "hkdf expand".to_string())
}

/// The content encryption key and nonce for one message (RFC 8291 §3.3 and §3.4, RFC 8188 §2.2-2.3), from the ECDH
/// secret, the phone's auth secret, both public keys and the salt.
pub fn derive(
    ecdh: &[u8],
    auth: &[u8],
    ua_public: &[u8],
    as_public: &[u8],
    salt: &[u8],
) -> Result<([u8; 16], [u8; 12]), String> {
    let prk_key = hkdf::Salt::new(hkdf::HKDF_SHA256, auth).extract(ecdh);
    let mut ikm = [0u8; 32];
    expand(
        &prk_key,
        &[b"WebPush: info\0", ua_public, as_public],
        &mut ikm,
    )?;
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, salt).extract(&ikm);
    let mut cek = [0u8; 16];
    expand(&prk, &[b"Content-Encoding: aes128gcm\0"], &mut cek)?;
    let mut nonce = [0u8; 12];
    expand(&prk, &[b"Content-Encoding: nonce\0"], &mut nonce)?;
    Ok((cek, nonce))
}

/// The one record: the plaintext, the last-record delimiter 0x02, sealed with its 16-octet tag.
pub fn seal(cek: &[u8; 16], nonce: [u8; 12], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key = aead::UnboundKey::new(&aead::AES_128_GCM, cek).map_err(|_| "aes key")?;
    let key = aead::LessSafeKey::new(key);
    let mut record = Vec::with_capacity(plaintext.len() + 17);
    record.extend_from_slice(plaintext);
    record.push(2);
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::empty(),
        &mut record,
    )
    .map_err(|_| "aes seal")?;
    Ok(record)
}

/// The `aes128gcm` header (RFC 8188 §2.1): salt, record size, and this message's public key as the key id.
pub fn header(salt: &[u8; 16], record_size: u32, as_public: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(21 + as_public.len());
    out.extend_from_slice(salt);
    out.extend_from_slice(&record_size.to_be_bytes());
    out.push(as_public.len() as u8);
    out.extend_from_slice(as_public);
    out
}

/// A body for the phone whose subscription keys are `ua_public` (its `p256dh`, 65 octets) and `auth` (16 octets):
/// a fresh P-256 key and salt for this message only, then `derive`, `seal` and `header`.
pub fn encrypt(ua_public: &[u8], auth: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    if plaintext.len() > PLAINTEXT_MOST {
        return Err("a push body over 3993 bytes".into());
    }
    let rng = SystemRandom::new();
    let mine = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng)
        .map_err(|_| "a P-256 key could not be made")?;
    let as_public = mine
        .compute_public_key()
        .map_err(|_| "a P-256 public key")?
        .as_ref()
        .to_vec();
    let mut salt = [0u8; 16];
    rng.fill(&mut salt).map_err(|_| "no random salt")?;
    let theirs = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, ua_public);
    let ecdh = agreement::agree_ephemeral(mine, &theirs, <[u8]>::to_vec)
        .map_err(|_| "the phone's p256dh is not a P-256 point")?;
    let (cek, nonce) = derive(&ecdh, auth, ua_public, &as_public, &salt)?;
    let mut body = header(&salt, RECORD_SIZE, &as_public);
    body.extend(seal(&cek, nonce, plaintext)?);
    Ok(body)
}

/// This PC's VAPID key (RFC 8292): an ECDSA P-256 key made once and kept in the session's data dir (`alerts.rs`).
/// The public half is what the page subscribes with; the private half never leaves this struct, and its `Debug`
/// prints the public half only, so no log line can carry it.
pub struct Vapid {
    pair: signature::EcdsaKeyPair,
}

impl fmt::Debug for Vapid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vapid(public {})", self.public_b64())
    }
}

impl Vapid {
    /// A new key, as PKCS#8 v1 bytes, for the key file.
    pub fn generate() -> Result<Vec<u8>, String> {
        signature::EcdsaKeyPair::generate_pkcs8(
            &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &SystemRandom::new(),
        )
        .map(|document| document.as_ref().to_vec())
        .map_err(|_| "a VAPID key could not be made".into())
    }

    pub fn from_pkcs8(pkcs8: &[u8]) -> Result<Self, String> {
        signature::EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            pkcs8,
            &SystemRandom::new(),
        )
        .map(|pair| Self { pair })
        .map_err(|_| "the VAPID key file is not a P-256 key".into())
    }

    /// The uncompressed public point, 65 octets, as the page's `applicationServerKey` takes it.
    pub fn public_key(&self) -> &[u8] {
        self.pair.public_key().as_ref()
    }

    pub fn public_b64(&self) -> String {
        b64(self.public_key())
    }

    /// A signed ES256 JWT for the push service at `audience` (its origin), good until `expires` (Unix seconds).
    pub fn token(&self, audience: &str, expires: u64) -> Result<String, String> {
        let head = b64(br#"{"typ":"JWT","alg":"ES256"}"#);
        let claims = serde_json::json!({ "aud": audience, "exp": expires, "sub": SUBJECT });
        let unsigned = format!("{head}.{}", b64(claims.to_string().as_bytes()));
        let signed = self
            .pair
            .sign(&SystemRandom::new(), unsigned.as_bytes())
            .map_err(|_| "the VAPID token could not be signed")?;
        Ok(format!("{unsigned}.{}", b64(signed.as_ref())))
    }

    /// The `Authorization` value for a push to `audience` (RFC 8292 §3): `vapid t=<token>, k=<public key>`.
    pub fn authorization(&self, audience: &str, now: u64) -> Result<String, String> {
        Ok(format!(
            "vapid t={}, k={}",
            self.token(audience, now + TOKEN_FOR)?,
            self.public_b64()
        ))
    }
}
