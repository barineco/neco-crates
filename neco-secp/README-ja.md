# neco-secp

最小限の secp256k1 / Nostr 署名コアをまとめた crate です。

秘密鍵型、x-only 公開鍵、Schnorr 署名、Nostr イベントの署名と検証に責務を絞り、ストレージ、ブラウザー連携、鍵隔離ポリシーはこの crate の外に置きます。

## 機能

- `nip19`: ベア形式 NIP-19 (`npub`, `nsec`, `note`) と TLV エンティティ (`nprofile`, `nevent`, `naddr`, `nrelay`) のエンコード / デコード
- `nip04`: NIP-04 の暗号化・復号補助
- `nip44`: NIP-44 v2 の会話鍵生成、暗号化、復号補助
- `serde`: イベント構造体のシリアライズ / デシリアライズ derive
- `nostr`: Nostr イベント補助関数と、署名済みイベント JSON 境界の有効化
- `batch`: 一括鍵生成とマイニング補助の有効化

## 使い方

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

`KeyBundle` は秘密鍵と x-only 公開鍵をひとまとめにした型です。

```rust
use neco_secp::KeyBundle;

let bundle = KeyBundle::generate().unwrap();
let secret = bundle.secret();
let xonly = bundle.xonly_public_key();
# let _ = (secret, xonly);
```

`nip19` 機能フラグを有効にすると npub / nsec の文字列を直接取得できます。

```rust
use neco_secp::{mine_pow, KeyBundle};

let bundle = KeyBundle::generate().unwrap();
let npub = bundle.npub().unwrap();
let nsec = bundle.nsec().unwrap();
```

`batch` 機能フラグを有効にすると一括生成とマイニングが使えます。

```rust
use neco_secp::KeyBundle;

// 一括鍵生成
let bundles = KeyBundle::generate_batch(100).unwrap();

// PoW マイニング: xonly pubkey の先頭 N nibble がゼロの鍵を探索
let bundle = mine_pow(4, 1_000_000).unwrap();
```

vanity マイニングは `batch` と `nip19` の両方が必要です。

```rust
use neco_secp::mine_vanity_npub;

// npub が指定プレフィックスで始まる鍵を探索
let bundle = mine_vanity_npub("npub1abc", 1_000_000).unwrap();
```

NIP-19 エンティティは `nip19` 機能フラグで有効になります。

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

NIP-44 補助関数は `nip44` 機能フラグで有効になります。

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

NIP-04 補助関数は `nip04` 機能フラグで有効になります。

```rust
use neco_secp::{nip04, SecretKey};

let secret = SecretKey::from_bytes([0x11; 32]).unwrap();
let peer = SecretKey::from_bytes([0x22; 32]).unwrap();
let peer_pubkey = peer.xonly_public_key().unwrap();
let payload = nip04::encrypt(&secret, &peer_pubkey, "hello", Some([0x44; 16])).unwrap();
let plaintext = nip04::decrypt(&peer, &secret.xonly_public_key().unwrap(), &payload).unwrap();

assert_eq!(plaintext, "hello");
```

## テスト

通常の `cargo test` では、時間のかかるマイニング系 test は既定で省かれます。

- vanity マイニングの stress test は `AINE_RUN_VANITY_TESTS=1` を付けたときだけ走る
- PoW マイニングの stress test は `AINE_RUN_POW_TESTS=1` を付けたときだけ走る

実行例:

```bash
# 通常の test
cargo test -p neco-secp

# 重い PoW マイニング test も実行
AINE_RUN_POW_TESTS=1 cargo test -p neco-secp --features batch

# 重い vanity マイニング test も実行
AINE_RUN_VANITY_TESTS=1 cargo test -p neco-secp --features "batch nip19"
```

## ECDSA 署名

`sign_ecdsa_prehash` / `verify_ecdsa_prehash` は Schnorr とは独立した K-256 ECDSA インターフェースです。

```rust
use neco_secp::{SecretKey, EcdsaSignature};

let secret = SecretKey::generate().unwrap();
let public = secret.public_key().unwrap();
let digest: [u8; 32] = [0x42; 32]; // SHA-256 ダイジェスト

let sig: EcdsaSignature = secret.sign_ecdsa_prehash(digest).unwrap();
public.verify_ecdsa_prehash(digest, &sig).unwrap();
```

署名側は low-S 正規化を行い、検証側は high-S 署名を拒否します。

## API

| 項目 | 説明 |
|------|------|
| `SecretKey` | 検証済み 32 バイトの secp256k1 秘密鍵 |
| `XOnlyPublicKey` | 検証済み 32 バイトの x-only 公開鍵 |
| `SchnorrSignature` | 64 バイトの Schnorr 署名 |
| `EcdsaSignature` | 64 バイトの ECDSA 署名（raw r\|\|s compact 形式）|
| `SecretKey::sign_ecdsa_prehash(digest)` | K-256 ECDSA prehash 署名（low-S 正規化） |
| `PublicKey::verify_ecdsa_prehash(digest, sig)` | K-256 ECDSA prehash 検証（high-S 拒否） |
| `KeyBundle` | 秘密鍵と x-only 公開鍵のペア |
| `KeyBundle::generate()` | ランダムな鍵ペアを生成する |
| `KeyBundle::secret()` / `xonly_public_key()` | 検証済みの秘密鍵と x-only 公開鍵を参照で返す |
| `KeyBundle::npub()` | npub 文字列を返す (`nip19`) |
| `KeyBundle::nsec()` | nsec 文字列を返す (`nip19`) |
| `KeyBundle::generate_batch(n)` | `n` 個の鍵ペアを一括生成する (`batch`) |
| `mine_pow(d, n)` | x-only 公開鍵の先頭 `d` ニブルをゼロにする鍵を探索する (`batch`) |
| `mine_vanity_npub(p, n)` | npub が `p` で始まる鍵を探索する (`batch` + `nip19`) |
| `UnsignedEvent` | 未署名 Nostr のイベント内容 |
| `SignedEvent` | 署名済み Nostr のイベント |
| `nostr` | イベントのシリアライズ、署名、検証補助 |
| `nip19` | bare/TLV NIP-19 のエンコード・デコード補助 |
| `nip04` | NIP-04 のイベント本文補助 |
| `nip44` | NIP-44 v2 の共有鍵・イベント本文補助 |

## ライセンス

MIT
