# neco-nostr-wasm

[English](README.md)

[neco-secp](../neco-secp) と [neco-vault](../neco-vault) の WebAssembly バインディングです。

Nostr 署名、vault 経由の暗号化、NIP 補助関数、採掘補助を `wasm-bindgen` 経由で JavaScript に公開します。境界は主に hex 文字列、JSON 文字列、暗号化済みバイト列ですが、JavaScript 側の相互運用のために `decode_nsec` とバニティ採掘結果は平文秘密鍵を返します。

## JavaScript バインディング

- `NostrSigner`: アカウント管理、署名、NIP-04/44、NIP-17、NIP-42、暗号化インポート・エクスポート、セキュリティ設定をまとめた vault ラッパー
- 関数: bech32 補助、イベント補助、採掘補助

## 境界規約

- 鍵: hex 文字列
- イベント: JSON 文字列
- 暗号化エクスポート: `Uint8Array`
- エラー: 文字列メッセージを持つ `JsValue`
- 平文秘密鍵の返却: `decode_nsec` とバニティ採掘結果

## 使い方

まず `wasm-pack build --target web` でビルドし、生成されたパッケージを JavaScript からインポートします。

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

| 項目 | 説明 |
|------|------|
| `NostrSigner` | 保管庫を使う署名・暗号化ラッパー |
| `encode_npub`, `encode_note`, `encode_nevent`, `encode_nprofile`, `encode_naddr`, `encode_nsec`, `decode_nsec`, `decode_bech32`, `encode_lnurl` | bech32 と NIP-19 の補助関数 |
| `derive_public_key`, `derive_public_key_sec1`, `parse_public_key_hex` | 公開鍵変換の補助関数 |
| `finalize_event`, `verify_event`, `serialize_event`, `get_event_hash` | Nostr イベント補助関数 |
| `validate_auth_event` | NIP-42 検証補助関数 |
| `generate_keypair`, `generate_keypairs_batch`, `mine_pow_batch` | 秘密鍵を返さない公開情報中心の生成 / 採掘補助 |
| `mine_vanity_batch`, `mine_vanity_with_candidates` | `secret_hex` を返すバニティ採掘補助 |

## ライセンス

MIT
