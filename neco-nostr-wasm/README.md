# neco-nostr-wasm

[日本語](README-ja.md)

WebAssembly bindings for [neco-secp](../neco-secp) and [neco-vault](../neco-vault).

This crate exposes Nostr signing, vault-backed encryption, NIP helpers, and mining helpers to JavaScript through `wasm-bindgen`. The boundary is mostly fixed to hex strings, JSON strings, and encrypted bytes. Plaintext secret keys are still intentionally exposed by `decode_nsec` and by vanity mining results, because those interop paths are sometimes needed in JavaScript callers.

## JavaScript binding surface

- `NostrSigner`: vault-backed signer with account management, signing, NIP-04/44, NIP-17, NIP-42, encrypted import/export, and security config
- free functions: bech32 helpers, event finalize / verify / hash helpers, and mining helpers

## Boundary rules

- keys: hex strings
- events: JSON strings
- encrypted export: `Uint8Array`
- errors: `Result<_, JsValue>` with string messages
- plaintext secret output: `decode_nsec` and vanity mining results

## Usage

Build with `wasm-pack build --target web`, then import the generated package from JavaScript.

```js
import init, { NostrSigner, encode_npub, mine_vanity_batch } from "./pkg/neco_nostr_wasm.js";

await init();

const signer = new NostrSigner(300);
signer.addAccountWithPlaintext("main", "<secret-hex>");

const signed = signer.signEvent(
  JSON.stringify({
    created_at: 1,
    kind: 1,
    tags: [],
    content: "hello",
  }),
);

const npub = encode_npub("<xonly-pubkey-hex>");
const vanity = JSON.parse(await mine_vanity_batch("bar", 100000));
```

## API

| Item | Description |
|------|-------------|
| `NostrSigner` | Vault-backed signer and encryption wrapper |
| `encode_npub`, `encode_note`, `encode_nevent`, `encode_nprofile`, `encode_naddr`, `encode_nsec`, `decode_nsec`, `decode_bech32`, `encode_lnurl` | Bech32 and NIP-19 helpers |
| `derive_public_key`, `derive_public_key_sec1`, `parse_public_key_hex` | Public-key conversion helpers |
| `finalize_event`, `verify_event`, `serialize_event`, `get_event_hash` | Nostr event helpers |
| `validate_auth_event` | NIP-42 validation helper |
| `generate_keypair`, `generate_keypairs_batch`, `mine_pow_batch` | Public mining / generation helpers without secret export |
| `mine_vanity_batch`, `mine_vanity_with_candidates` | Vanity mining helpers that return `secret_hex` |

## License

MIT
