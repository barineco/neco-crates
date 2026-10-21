# neco crates

[English](README.md)

`neco crates` は、幾何処理・数値計算・可視化に関連する Rust の crate 群です。

単一のアプリケーションで共有していた計算機能を、個別に再利用できる crate として提供します。各 crate の責務を分け、必要な機能を選んで組み合わせられる構成です。収録分野は、計算幾何・スプラインと NURBS・疎行列と固有値計算・クラスタリング・色彩と顔料モデル・STL とメッシュ処理・二次元のビュー操作です。

## crate 一覧

外部依存は、常時使うものを先に書き、任意機能で有効になるものを括弧内にまとめています。`serde` は任意機能で有効にできるシリアライズとデシリアライズへの対応を示します。

JSON と KDL の専用パーサーおよびシリアライザは、次のリポジトリで提供します。

- リポジトリ: [`barineco/neco-parser`](https://github.com/barineco/neco-parser)
- 対象の crate: `neco-json`, `neco-kdl`

### 幾何とメッシュ

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-nurbs`](./neco-nurbs) | NURBS 曲線・曲面、フィッティング、多項式補助 | なし | （`nalgebra`） |
| [`neco-brep`](./neco-brep) | B-rep、立体構築、テセレーション、3D ブール演算 | `neco-nurbs`, `neco-cdt` | なし |
| [`neco-mesh`](./neco-mesh) | 2D / 3D メッシュ生成とメッシュ処理 | `neco-cdt`, `neco-nurbs`, `neco-stl` | （`serde`） |
| [`neco-stl`](./neco-stl) | STL の読み書き | なし | なし |
| [`neco-cdt`](./neco-cdt) | 制約付き Delaunay 三角形分割 | なし | なし |
| [`neco-spline`](./neco-spline) | スプライン補間 | なし | なし |

### 行列計算と数値解法

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-array2`](./neco-array2) | 格子系 crate 向けの軽量行優先 2D 配列基盤 | なし | （`serde`） |
| [`neco-gridfield`](./neco-gridfield) | 一様 2D 格子と時間発展向けの三重バッファ状態管理 | `neco-array2` | （`serde`） |
| [`neco-contact`](./neco-contact) | 一様 2D 場向けの Hertz 接触と空間補助機能 | `neco-array2` | なし |
| [`neco-dop853`](./neco-dop853) | 適応刻み Dormand-Prince 8(5,3) ODE 積分 | なし | なし |
| [`neco-stencil`](./neco-stencil) | 一様 2D 格子向けの差分ステンシル演算 | なし | （`rayon`） |

### 信号処理

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-stft`](./neco-stft) | 実数の周波数変換、窓関数、短時間周波数変換 | なし | 別リポジトリの線形代数クレート |
| [`neco-minphase`](./neco-minphase) | 最小位相スペクトル、インパルス応答、重ね合わせ加算 | `neco-stft` | 別リポジトリの線形代数クレート |

### クラスタリング

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-kmeans`](./neco-kmeans) | k-means クラスタリング | なし | （`rayon`） |
| [`neco-spectral`](./neco-spectral) | スペクトラルクラスタリング | `neco-kmeans` | 別リポジトリの線形代数クレート |

### 検索と順位付け

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-fuzzy`](./neco-fuzzy) | コマンド、パス、短い識別子向けの最小 fuzzy スコアコア | なし | なし |

### エンコーディングとデータ形式

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-base58`](./neco-base58) | Base58BTC エンコーダとデコーダ | なし | なし |
| [`neco-base64`](./neco-base64) | Base64 エンコーダとデコーダ | なし | なし |
| [`neco-cid`](./neco-cid) | CIDv1 とマルチベースコア | `neco-sha2` | なし |
| [`neco-cbor`](./neco-cbor) | `no_std` 環境向け CBOR / DAG-CBOR コーデック | `neco-base64`, `neco-cid`, `neco-json` (別 repo) | なし |
| [`neco-car`](./neco-car) | コンテンツアドレス可能アーカイブ向け CAR v1 パーサーとライター | `neco-cbor`, `neco-cid` | なし |

### 暗号処理

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-sha1`](./neco-sha1) | SHA-1 ハッシュ関数 | なし | なし |
| [`neco-sha2`](./neco-sha2) | SHA-256、HMAC-SHA256、HKDF-SHA256 | なし | なし |
| [`neco-gf256`](./neco-gf256) | GF(2^8) 有限体演算 | なし | なし |
| [`neco-galois`](./neco-galois) | secp256k1 / P-256 向けの有限体演算 | `neco-sha2` | なし |
| [`neco-ecc`](./neco-ecc) | GF(2^8) 上の Reed-Solomon 誤り訂正 | `neco-gf256` | なし |
| [`neco-rand`](./neco-rand) | 決定論的な非暗号乱数生成と安定バケット割り当て | なし | なし |
| [`neco-p256`](./neco-p256) | P-256 ECDSA 署名コア | `neco-galois`, `neco-sha2` | `getrandom` |
| [`neco-secp`](./neco-secp) | 最小限の secp256k1 / Nostr 署名コア | なし | `k256`, `sha2`（`serde_json`, `bech32`, `aes`, `cbc`, `chacha20`, `hkdf`, `hmac`, `base64`） |
| [`neco-argon2`](./neco-argon2) | Blake2b ベースの Argon2id パスワードハッシュ | `neco-base64` | `getrandom` |
| [`neco-vault`](./neco-vault) | `neco-secp` 上で動作するインメモリ署名保管庫 | `neco-secp` | なし（`aes`, `cbc`, `scrypt`, `getrandom`, `sha2`） |
| [`neco-nostr-wasm`](./neco-nostr-wasm) | `neco-secp` と `neco-vault` の WebAssembly バインディング | `neco-secp`, `neco-vault` | `bech32`, `serde_json`, `wasm-bindgen` |

### 音響解析

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-radiation`](./neco-radiation) | 振動面と板モード向けの音響放射パワー推定 | なし | （`serde`） |
| [`neco-modal`](./neco-modal) | 振動信号向けのモード抽出と軽量なモード集合補助 | `neco-stft` | なし |

### 色処理

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-color`](./neco-color) | 色空間と測色のユーティリティ | なし | なし |
| [`neco-pigment`](./neco-pigment) | 顔料寄りの分光・混色ユーティリティ | `neco-color` | （`serde`） |

### ノードグラフ

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-nodegraph`](./neco-nodegraph) | 描画非依存のノードグラフモデル | ( `neco-json` 別 repo ) | なし |
| [`neco-edge-routing`](./neco-edge-routing) | ノードグラフ向けの 2D エッジルーティング | （`neco-spline`, `neco-nurbs`） | なし |
| [`neco-edge-routing-wasm`](./neco-edge-routing-wasm) | `neco-edge-routing` の WebAssembly バインディング | `neco-edge-routing` | `wasm-bindgen`, `js-sys` |

### データ構造

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-tree`](./neco-tree) | カーソルベース操作の汎用木構造 | なし | なし |

### ビュー操作とバインディング

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-view2d`](./neco-view2d) | 2D カメラ / ビューポート操作 | なし | （`serde`） |
| [`neco-view2d-svg`](./neco-view2d-svg) | `neco-view2d` のワールド座標を SVG 属性文字列へ変換 | `neco-view2d` | なし |
| [`neco-view2d-svg-wasm`](./neco-view2d-svg-wasm) | `neco-view2d-svg` の WebAssembly バインディング | `neco-view2d-svg`, `neco-view2d` | `wasm-bindgen` |
| [`neco-view2d-wasm`](./neco-view2d-wasm) | `neco-view2d` の WebAssembly バインディング | `neco-view2d` | `wasm-bindgen` |

### CLI

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-tui`](./neco-tui) | 最小限の ANSI ターミナル補助 | なし | なし |
| [`neco-argparse`](./neco-argparse) | `neco-json` ベースの CLI 引数パーサー | `neco-json` (別 repo) | なし |

多くの crate は、crates.io で個別に公開できる独立性を保っています。一つのリポジトリで管理しつつ、利用時には必要なものだけを組み合わせられます。

このリポジトリは開発中であり、crate や実装箇所ごとに成熟度が異なります。実用できる機能に加え、機能追加や構成の整理を続けている部分も含まれます。

内部実装は、関数の内製化・アルゴリズムの置換・高速化に伴って変更されることがあります。公開 API は、版の更新内容を確認して利用してください。

## 状況

- 品質検査: 整形・静的検査・テスト
- 継続的検査: [`.github/workflows/ci.yml`](./.github/workflows/ci.yml)
- 更新単位: crate ごとの成熟度と版

## コントリビューション

課題報告とプルリクエストを受け付けています。対象と目的を明確にした変更は、影響範囲を確認しやすくなります。

詳しい案内は次の文書にあります。

- 開発手順: [CONTRIBUTING.md](./CONTRIBUTING.md)
- 脆弱性の報告方法: [SECURITY.md](./SECURITY.md)

## サポート

この crate 群や関連アプリケーションが役に立った場合は、次のページから継続開発を支援できます。

- OFUSE: <https://ofuse.me/barineco>
- Ko-fi: <https://ko-fi.com/barineco>

支援は保守、安定化対応、機能追加の継続に充てます。

## ライセンス

特記がない限り、このリポジトリは [MIT ライセンス](./LICENSE) です。
