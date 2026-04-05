# neco-secp

Minimal secp256k1 and Nostr signing core, for building Nostr clients and tools in pure Rust.

This crate provides a small pure Rust core for secret keys, x-only public keys, Schnorr signatures, and Nostr event signing. Storage, browser integration, and key isolation policy are handled by adjacent higher-level crates.

## Features

- `nip19`: enable NIP-19 bare entities (`npub`, `nsec`, `note`) and TLV entities (`nprofile`, `nevent`, `naddr`, `nrelay`)
- `nip04`: enable NIP-04 encryption/decryption helpers
- `nip44`: enable NIP-44 v2 conversation key, encryption, and decryption helpers
- `serde`: derive serialization for event structs
- `nostr`: enable Nostr event helpers and signed-event JSON support
- `batch`: enable batch key generation and mining helpers

## Usage

```rust
use neco_secp::{nostr, SecretKey, UnsignedEvent};

let secret = SecretKey::generate().unwrap();
let event = UnsignedEvent {
    created_at: 1_700_000_000,
    kind: 1,
    tags: vec![],
    content: "hello".to_string(),
};

let signed = nostr::finalize_event(event, &secret).unwrap();
nostr::verify_event(&signed).unwrap();
```

`KeyBundle` bundles a secret key and its x-only public key together.

```rust
use neco_secp::KeyBundle;

let bundle = KeyBundle::generate().unwrap();
let secret = bundle.secret();
let xonly = bundle.xonly_public_key();
# let _ = (secret, xonly);
```

With `nip19` feature:

```rust
use neco_secp::{mine_pow, KeyBundle};

let bundle = KeyBundle::generate().unwrap();
let npub = bundle.npub().unwrap();
let nsec = bundle.nsec().unwrap();
```

`batch` feature enables generating multiple key pairs at once, and mining helpers.

```rust
use neco_secp::KeyBundle;

// batch key generation
let bundles = KeyBundle::generate_batch(100).unwrap();

// PoW mining: find a key with N leading zero hex nibbles in the xonly pubkey
let bundle = mine_pow(4, 1_000_000).unwrap();
```

Vanity mining requires both `batch` and `nip19` features.

```rust
use neco_secp::mine_vanity_npub;

// find a key whose npub starts with the given prefix
let bundle = mine_vanity_npub("npub1abc", 1_000_000).unwrap();
```

NIP-19 entities are available behind the `nip19` feature.

```rust
use neco_secp::{nip19, NProfile, SecretKey};

let secret = SecretKey::generate().unwrap();
let nsec = nip19::encode_nsec(&secret).unwrap();
let decoded = nip19::decode(&nsec).unwrap();

assert!(matches!(decoded, neco_secp::Nip19::Nsec(_)));

let profile = NProfile {
    pubkey: secret.xonly_public_key().unwrap(),
    relays: vec!["wss://relay.example".to_string()],
};
let nprofile = nip19::encode_nprofile(&profile).unwrap();
let decoded = nip19::decode(&nprofile).unwrap();

assert!(matches!(decoded, neco_secp::Nip19::NProfile(_)));
```

NIP-44 helpers are available behind the `nip44` feature.

```rust
use neco_secp::{nip44, SecretKey};

let secret = SecretKey::from_bytes([0x11; 32]).unwrap();
let peer = SecretKey::from_bytes([0x22; 32]).unwrap();
let peer_pubkey = peer.xonly_public_key().unwrap();
let conversation_key = nip44::get_conversation_key(&secret, &peer_pubkey).unwrap();
let payload = nip44::encrypt("hello", &conversation_key, Some([0x33; 32])).unwrap();
let plaintext = nip44::decrypt(&payload, &conversation_key).unwrap();

assert_eq!(plaintext, "hello");
```

NIP-04 helpers are available behind the `nip04` feature.

```rust
use neco_secp::{nip04, SecretKey};

let secret = SecretKey::from_bytes([0x11; 32]).unwrap();
let peer = SecretKey::from_bytes([0x22; 32]).unwrap();
let peer_pubkey = peer.xonly_public_key().unwrap();
let payload = nip04::encrypt(&secret, &peer_pubkey, "hello", Some([0x44; 16])).unwrap();
let plaintext = nip04::decrypt(&peer, &secret.xonly_public_key().unwrap(), &payload).unwrap();

assert_eq!(plaintext, "hello");
```

## Testing

Normal `cargo test` runs don't include expensive mining checks.

- Vanity mining stress tests run only when `AINE_RUN_VANITY_TESTS=1` is set.
- PoW mining stress tests run only when `AINE_RUN_POW_TESTS=1` is set.

Examples:

```bash
# run ordinary tests
cargo test -p neco-secp

# include expensive PoW mining checks
AINE_RUN_POW_TESTS=1 cargo test -p neco-secp --features batch

# include expensive vanity mining checks
AINE_RUN_VANITY_TESTS=1 cargo test -p neco-secp --features "batch nip19"
```

## ECDSA Signing

`sign_ecdsa_prehash` / `verify_ecdsa_prehash` provide a K-256 ECDSA interface independent of Schnorr.

```rust
use neco_secp::{SecretKey, EcdsaSignature};

let secret = SecretKey::generate().unwrap();
let public = secret.public_key().unwrap();
let digest: [u8; 32] = [0x42; 32]; // SHA-256 digest

let sig: EcdsaSignature = secret.sign_ecdsa_prehash(digest).unwrap();
public.verify_ecdsa_prehash(digest, &sig).unwrap();
```

The signing side applies low-S normalization; the verification side rejects high-S signatures.

## API

| Item | Description |
|------|-------------|
| `SecretKey` | Validated 32-byte secp256k1 secret key |
| `XOnlyPublicKey` | Validated 32-byte x-only public key |
| `SchnorrSignature` | 64-byte Schnorr signature |
| `EcdsaSignature` | 64-byte ECDSA signature (raw r\|\|s compact format) |
| `SecretKey::sign_ecdsa_prehash(digest)` | K-256 ECDSA prehash signing with low-S normalization |
| `PublicKey::verify_ecdsa_prehash(digest, sig)` | K-256 ECDSA prehash verification with high-S rejection |
| `KeyBundle` | Bundled secret key and x-only public key pair |
| `KeyBundle::generate()` | Generates a random key pair |
| `KeyBundle::secret()` / `xonly_public_key()` | Borrow the validated secret key and x-only public key |
| `KeyBundle::npub()` | Returns the npub string (`nip19`) |
| `KeyBundle::nsec()` | Returns the nsec string (`nip19`) |
| `KeyBundle::generate_batch(n)` | Generates `n` key pairs (`batch`) |
| `mine_pow(d, n)` | Finds a key with `d` leading zero hex nibbles (`batch`) |
| `mine_vanity_npub(p, n)` | Finds a key whose npub starts with `p` (`batch` + `nip19`) |
| `UnsignedEvent` | Unsigned Nostr event payload |
| `SignedEvent` | Signed Nostr event |
| `nostr` | Event serialization, signing, and verification helpers |
| `nip19` | Bare/TLV NIP-19 encode/decode helpers |
| `nip04` | NIP-04 payload helpers |
| `nip44` | NIP-44 v2 conversation key and payload helpers |

## License

MIT
