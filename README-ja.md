# neco crates

[English](README.md)

`neco crates` は、幾何処理、数値計算、可視化に関連する Rust crate 群です。

単一アプリケーション内で共通化していた計算基盤を独立して再利用できる crate へ切り出し、巨大な一体型フレームワークにはせず責務を分離する方針で、現在は計算幾何、スプライン / NURBS、疎行列と固有値計算、クラスタリング、色彩と顔料モデル、STL / メッシュ処理、2D ビュー操作を収録しています。

## crate 一覧

外部依存は、常時依存を先に書き、optional な依存を括弧内にまとめています。
`serde` の表記は opt-in な `Serialize` / `Deserialize` 対応を示します。JSON 専用の入出力は [`neco-json`](./neco-json) が担います。

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
| [`neco-complex`](./neco-complex) | FFT 系とソルバー連携向けの軽量複素数基盤 | なし | なし |
| [`neco-gridfield`](./neco-gridfield) | 一様 2D 格子と時間発展向けの三重バッファ状態管理 | `neco-array2` | （`serde`） |
| [`neco-contact`](./neco-contact) | 一様 2D 場向けの Hertz 接触と空間補助機能 | `neco-array2` | なし |
| [`neco-sparse`](./neco-sparse) | 疎行列データ構造 | なし | なし |
| [`neco-eigensolve`](./neco-eigensolve) | 疎行列向け固有値ソルバ | `neco-sparse` | （`rayon`, `faer`） |
| [`neco-dop853`](./neco-dop853) | 適応刻み Dormand-Prince 8(5,3) ODE 積分 | なし | なし |
| [`neco-stencil`](./neco-stencil) | 一様 2D 格子向けの差分ステンシル演算 | なし | （`rayon`） |

### 信号処理

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-stft`](./neco-stft) | バックエンド非依存の実数 FFT ファサード、窓関数、STFT / ISTFT | `neco-complex` | なし |
| [`neco-minphase`](./neco-minphase) | 最小位相スペクトル・インパルスカーネルの重ね合わせ加算法（OLA） | `neco-stft`, `neco-complex` | なし |

### クラスタリング

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-kmeans`](./neco-kmeans) | k-means クラスタリング | なし | （`rayon`） |
| [`neco-spectral`](./neco-spectral) | スペクトラルクラスタリング | `neco-sparse`, `neco-eigensolve`, `neco-kmeans` | なし |

### 検索と順位付け

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-fuzzy`](./neco-fuzzy) | コマンド、パス、短い識別子向けの最小 fuzzy スコアコア | なし | なし |

### 暗号処理

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-rand`](./neco-rand) | 決定論的な非暗号乱数生成と安定バケット割り当て | なし | なし |
| [`neco-secp`](./neco-secp) | 最小限の secp256k1 / Nostr 署名コア | なし | `k256`, `sha2`（`serde_json`, `bech32`, `aes`, `cbc`, `chacha20`, `hkdf`, `hmac`, `base64`） |
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
| [`neco-nodegraph`](./neco-nodegraph) | 描画非依存のノードグラフモデル | （`neco-json`） | なし |
| [`neco-edge-routing`](./neco-edge-routing) | ノードグラフ向けの 2D エッジルーティング | （`neco-spline`, `neco-nurbs`） | なし |

### ビュー操作とバインディング

| crate | 概要 | 内部依存 | 主な外部依存 |
|---|---|---|---|
| [`neco-view2d`](./neco-view2d) | 2D カメラ / ビューポート操作 | なし | （`serde`） |
| [`neco-view2d-svg`](./neco-view2d-svg) | `neco-view2d` のワールド座標を SVG 属性文字列へ変換 | `neco-view2d` | なし |
| [`neco-view2d-wasm`](./neco-view2d-wasm) | `neco-view2d` の WebAssembly バインディング | `neco-view2d` | `wasm-bindgen` |

大半のcrateは crates.io で個別公開できるよう、意図的に独立性を保っています。運用は monorepo 体制ですが、実行時に密結合する単一フレームワークではありません。

このリポジトリはまだ開発途中で、crate やコードパスごとに成熟度に差があります。すでに実用できる部分もありますが、まだ詰めている途中の実装や、機能追加・再整理を続けている部分も含みます。

更新では、内部実装の変更が比較的起こりやすい状態です。特に、関数の内製化、アルゴリズム差し替え、高速化を目的とした実装変更は、全 crate 一律の長期安定 API より起こりやすいものとして考えてください。

## 状況

- リポジトリ全体で整形、lint、テストのゲートを維持
- GitHub Actions CI は [`.github/workflows/ci.yml`](./.github/workflows/ci.yml) に設定
- crate ごとに成熟度や更新速度は異なる
- 古いコメント規約の不揃いなど、一部の体裁は未整理

## コントリビューション

課題報告と pull request は歓迎します。

広すぎる提案より、対象と目的が絞られた変更の方が検証しやすくなります。

開発フローは [CONTRIBUTING.md](./CONTRIBUTING.md)、脆弱性報告は [SECURITY.md](./SECURITY.md) を参照してください。

## サポート

このcrate群や関連アプリが役に立った場合は、次のページから継続開発を支援できます。

- OFUSE: <https://ofuse.me/barineco>
- Ko-fi: <https://ko-fi.com/barineco>

支援は保守、安定化対応、機能追加の継続に充てます。

## ライセンス

特記がない限り、このリポジトリは [MIT ライセンス](./LICENSE) です。
