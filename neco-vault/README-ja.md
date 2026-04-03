# neco-vault

`neco-secp` の上に載るインメモリ署名 vault です。

秘密鍵は vault 内部に閉じ込め、平文では外に出しません。外部からは署名と Nostr 暗号化だけが使えます。

設計意図と境界の説明は [ARCHITECTURE-ja.md](ARCHITECTURE-ja.md) にまとめています。

## 機能

- `nostr`: `neco-secp` 経由の Nostr 署名を有効化
- `nip04`: vault 経由の NIP-04 暗号化と復号を有効化
- `nip44`: vault 経由の NIP-44 暗号化／復号を有効化
- `nip17`: ギフトラップ DM 補助を有効化
- `encrypted`: scrypt 鍵導出付き AES-256-CBC による暗号化インポート／エクスポート（`aes`、`cbc`、`getrandom`、`scrypt` を追加）
- `encrypted-legacy-v1`: SHA-256（passphrase）ベースの v1 インポート互換を任意で有効化
- `security-hardening`: 擬似遅延、ダミー演算を含む堅牢化フックを有効化
- `wasm`: 将来のブラウザー / WebAssembly 連携の予約機能フラグ

## 設計方針

vault は平文の秘密鍵を返しません。秘密鍵を必要とする操作はすべて vault 内部で完結し、署名済みイベント、暗号化ペイロード、公開鍵など結果だけを返します。

## 使い方

### 基本的な署名

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

### アクティブアカウント

`sign_event_active` を使えばラベル指定なしで署名でき、最初にインポートしたアカウントは自動的にアクティブになります。

```rust
vault.import_plaintext("alice", secret, 100).unwrap();
// "alice" がアクティブになる

vault.set_active("alice").unwrap();
let label = vault.active_label(); // Some("alice")

let signed = vault.sign_event_active(event, 102).unwrap();
```

アクティブなアカウントを削除するとアクティブは `None` になります。

```rust
vault.remove("alice").unwrap();
assert_eq!(vault.active_label(), None);
```

`labels()` で保存済みの全ラベルを取得できます。

### 暗号化インポート／エクスポート（`encrypted` feature）

エクスポートでは scrypt 鍵導出と AES-256-CBC を使い、インポートは既定では v2 フォーマットのみを受け付けます。旧 SHA-256 鍵導出の v1 バイト列は `encrypted-legacy-v1` feature 有効時だけ受け付けます。

```rust
let data = vault.export_encrypted("alice", b"passphrase").unwrap();
vault.import_encrypted("bob", b"passphrase", &data, 200).unwrap();
```

### vault 経由の NIP-04 / NIP-44

```rust
let bob = vault.public_key("bob").unwrap();
let payload = vault.nip44_encrypt_active(&bob, "hello", 101).unwrap();
let text = vault.nip44_decrypt_active(&bob, &payload, 102).unwrap();
assert_eq!(text, "hello");
```

### セキュリティ強化（`security-hardening` 機能フラグ）

`SecurityConfig` で定数時間タッチ、ランダム遅延、ダミー演算を切り替えられます。

## API

| 項目 | 説明 |
|------|------|
| `SecurityConfig` | 秘密鍵利用経路のセキュリティ強化設定 |
| `VaultConfig` | キャッシュタイムアウトの設定 |
| `Vault` | インメモリ秘密鍵保持と署名の入口 |
| `VaultError` | 保管庫層のエラー型 |
| `Vault::import_plaintext` | ラベルに紐付けて秘密鍵をインポートする |
| `Vault::remove` | ラベルでアカウントを削除する |
| `Vault::labels` | 保存済み全ラベルを返す |
| `Vault::set_active` | アクティブアカウントを設定する |
| `Vault::active_label` | 現在のアクティブラベルを返す |
| `Vault::public_key` | 指定ラベルの x-only 公開鍵を返す |
| `Vault::public_key_active` | アクティブアカウントの x-only 公開鍵を返す |
| `Vault::set_security_config` | 実行時のセキュリティ強化設定を更新する |
| `Vault::security_config` | 現在のセキュリティ強化設定を返す |
| `Vault::sign_event` | 指定アカウントで Nostr イベントに署名する |
| `Vault::sign_event_active` | アクティブアカウントで Nostr イベントに署名する |
| `Vault::nip04_encrypt`, `Vault::nip04_decrypt` | NIP-04 の保管庫経由暗号化・復号 |
| `Vault::nip44_encrypt`, `Vault::nip44_decrypt` | NIP-44 の保管庫経由暗号化・復号 |
| `Vault::create_sealed_dm`, `Vault::open_gift_wrap_dm` | NIP-17 の保管庫経由 DM 補助機能 |
| `Vault::export_encrypted` | 秘密鍵を AES-256-CBC 暗号化バイト列としてエクスポートする（`encrypted` 機能） |
| `Vault::import_encrypted` | 暗号化バイト列から秘密鍵をインポートする（`encrypted` 機能） |

## ライセンス

MIT
