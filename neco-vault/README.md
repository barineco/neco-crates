# neco-vault

Memory-only signing vault built on `neco-secp`, for Nostr client apps that need to sign events without exposing secret keys.

It's a higher-level layer: secret keys stay inside the vault and aren't returned by any public API — only signed events, ciphertext, and public keys come out.

A Japanese architecture note is available in [ARCHITECTURE-ja.md](ARCHITECTURE-ja.md).

## Features

- `nostr`: enable Nostr signing through `neco-secp`
- `nip04`: enable NIP-04 encrypt/decrypt through the vault
- `nip44`: enable NIP-44 encrypt/decrypt through the vault
- `nip17`: enable gift-wrap DM helpers through the vault
- `encrypted`: encrypted import/export via AES-256-CBC with scrypt-based key derivation (adds `aes`, `cbc`, `getrandom`, `scrypt` deps)
- `encrypted-legacy-v1`: opt-in v1 import compatibility via SHA-256(passphrase)
- `security-hardening`: enable optional random delay / dummy operation hardening hooks
- `wasm`: reserved feature for future browser / wasm integration

## Design

The vault never returns plaintext secret keys. All operations that need a secret key run inside the vault; only the result (a signed event, ciphertext, or public key) comes out.

## Usage

### Basic signing

```rust
use neco_secp::{SecretKey, UnsignedEvent};
use neco_vault::{Vault, VaultConfig};

let mut vault = Vault::new(VaultConfig::default()).unwrap();
let secret = SecretKey::generate().unwrap();

vault.import_plaintext("main", secret, 100).unwrap();

let signed = vault.sign_event(
    "main",
    UnsignedEvent {
        created_at: 101,
        kind: 1,
        tags: vec![],
        content: "hello".to_string(),
    },
    102,
).unwrap();

assert_eq!(signed.kind, 1);
```

### Active account

The first imported account is automatically set as active. Use `sign_event_active` to sign without specifying a label.

```rust
vault.import_plaintext("alice", secret, 100).unwrap();
// "alice" is now active

vault.set_active("alice").unwrap();
let label = vault.active_label(); // Some("alice")

let signed = vault.sign_event_active(event, 102).unwrap();
```

Removing the active account sets active to `None`.

```rust
vault.remove("alice").unwrap();
assert_eq!(vault.active_label(), None);
```

Use `labels()` to list all stored account labels.

### Encrypted import/export (`encrypted` feature)

Encrypted export uses scrypt-derived keys and AES-256-CBC. Import accepts only the v2 format by default; older SHA-256-derived v1 blobs don't load unless you enable the `encrypted-legacy-v1` feature.

```rust
let data = vault.export_encrypted("alice", b"passphrase").unwrap();
vault.import_encrypted("bob", b"passphrase", &data, 200).unwrap();
```

### NIP-04 / NIP-44 through the vault

```rust
let bob = vault.public_key("bob").unwrap();
let payload = vault.nip44_encrypt_active(&bob, "hello", 101).unwrap();
let text = vault.nip44_decrypt_active(&bob, &payload, 102).unwrap();
assert_eq!(text, "hello");
```

### Security hardening (`security-hardening` feature)

`SecurityConfig` lets callers enable constant-time touch points, random delay, and dummy operations for secret-key use paths.

## API

| Item | Description |
|------|-------------|
| `SecurityConfig` | Optional hardening flags for secret-key use paths |
| `VaultConfig` | Cache timeout configuration |
| `Vault` | Memory-only secret storage and signing entry point |
| `VaultError` | Vault-level error type |
| `Vault::import_plaintext` | Import a secret key under a label |
| `Vault::remove` | Delete an account by label |
| `Vault::labels` | Return all stored labels |
| `Vault::set_active` | Set the active account label |
| `Vault::active_label` | Return the current active label |
| `Vault::public_key` | Return the xonly public key for a label |
| `Vault::public_key_active` | Return the xonly public key for the active account |
| `Vault::set_security_config` | Update runtime hardening settings |
| `Vault::security_config` | Return current hardening settings |
| `Vault::sign_event` | Sign a Nostr event with a named account |
| `Vault::sign_event_active` | Sign a Nostr event with the active account |
| `Vault::nip04_encrypt`, `Vault::nip04_decrypt` | NIP-04 vault encryption helpers (`nip04` feature) |
| `Vault::nip44_encrypt`, `Vault::nip44_decrypt` | NIP-44 vault encryption helpers (`nip44` feature) |
| `Vault::create_sealed_dm`, `Vault::open_gift_wrap_dm` | NIP-17 vault DM helpers (`nip17` feature) |
| `Vault::export_encrypted` | Export a secret key as AES-256-CBC encrypted bytes (`encrypted` feature) |
| `Vault::import_encrypted` | Import a secret key from encrypted bytes (`encrypted` feature) |

## License

MIT
