# neco-modal

[English](README.md)

`neco-modal` は、振動や共振の時系列信号に対するモード抽出、軽量なモード集合保持、線形モーダル応答ユーティリティをまとめた crate です。

## モード抽出、保持、応答

`extract_modes` は、等間隔サンプリングした観測信号から主要なピークを拾い、周波数、減衰、振幅、位相、Q を推定します。

`ModalSet` は形状配列の長さや周波数を検証したうえでモード情報を保持し、ソルバー固有の付加情報は crate の外に出しています。

`DampedModalSet` は外部から与える減衰係数を重ねる拡張層です。線形インパルス応答の補助関数と `OscillatorBank` は、この減衰モデルを受け取って動きます。

## 使い方

```rust
use neco_modal::{extract_modes, ModalRecord, ModalSet, ShapeLayout};

let dt = 1.0 / 8_000.0;
let samples = 4096;
let readout: Vec<f64> = (0..samples)
    .map(|n| (2.0 * std::f64::consts::PI * 440.0 * n as f64 * dt).sin())
    .collect();

let extracted = extract_modes(&readout, dt, 4, -40.0);
let layout = ShapeLayout::new(1, 8)?;
let modal_set = ModalSet::new(
    vec![ModalRecord::new(
        extracted[0].freq,
        extracted[0].amplitude,
        vec![1.0; 8],
        Some(extracted[0].damping_rate),
        Some(extracted[0].quality),
    )],
    layout,
)?;

assert_eq!(modal_set.len(), 1);
# Ok::<(), neco_modal::ModalSetError>(())
```

## API

| 項目 | 説明 |
|------|------|
| `extract_modes(readout, dt, n_modes_max, threshold_db)` | Hann 窓と FFT スペクトルから主要ピークを拾い、観測信号から時間モードを抽出する |
| `extract_spatial_modes(snapshots, snapshot_times, mode_freqs, nx, ny)` | 指定周波数に対する空間モードの大きさをスナップショット群から復元する |
| `ModeInfo` | 周波数、減衰推定、振幅、位相、Q を持つ抽出結果 |
| `SpatialMode` | 復元した空間振幅と推定した周方向次数 |
| `ShapeLayout::new(dof_per_node, n_nodes)` | 平坦化した形状配列の配置情報を検証する |
| `ModalRecord::new(freq, weight, shape, observed_damping, quality)` | 軽量な単一モード記録を作る |
| `ModalSet::new(modes, layout)` | 共通の配置情報を持つ複数モードを検証付きで保持する |
| `ModalSet::filter_freq_min(f_min)` | `f_min` 以上の周波数だけを残す |
| `ModalSet::subset(indices)` | 添字指定で抜き出しや並べ替えを行う |
| `ModalSet::sorted_by_freq()` | 周波数昇順に並べた複製を返す |
| `ModalSet::merge(other)` | 配置情報が一致する 2 つのモード集合を結合し、周波数順を保つ |
| `ModalSet::normalized_shapes_l2()` | `weight` を変えずに形状の L2 ノルムを 1 にそろえた複製を返す |
| `ModalSet::component(index)` | 平坦化した形状配列から各節点の単一成分を抜き出す |
| `ModalSet::components(indices)` | 指定した成分だけで新しい形状配列を再構成する |
| `ModalSet::with_gammas(gammas)` | 外部減衰係数をコアから分離した拡張モデルとして上乗せする |
| `DampedModalRecord` / `DampedModalSet` | `gamma` をコアから分離して保持する拡張層の型 |
| `SourceExcitation::new(node, delay, gain, phase)` | マルチソースのモーダル応答や駆動更新に使う励起源を検証付きで作る |
| `compute_source_amps(modes, source_node, receiver_node, component)` | モード形状から観測点・受信点への射影をモードごとに作る |
| `compute_source_amps_multi(modes, excitations, component)` | 複数励起点向けの生射影をモードごとに作る |
| `generate_ir(modes, source_amps, duration, sample_rate)` | 事前計算した励起振幅から線形減衰モーダルインパルス応答を生成する |
| `generate_ir_multi(modes, excitations, receiver_node, duration, sample_rate, component)` | 励起記述から直接マルチソースの減衰モーダルインパルス応答を生成する |
| `OscillatorBank::new(modes, source_amps, sample_rate, component)` | 減衰モーダル集合と単一励起の射影から線形オシレーター銀行を作る |
| `OscillatorBank::new_multi(modes, excitations, sample_rate, component)` | 複数励起から線形オシレーター銀行を作る |
| `OscillatorBank::set_receiver(receiver_amps)` | 受信側の再構成重みを設定する |
| `OscillatorBank::process(output)` | 新規入力なしでオシレーター銀行を進めて音列を書き出す |
| `OscillatorBank::field_weights()` / `compute_field()` | 現在のモーダル重みか空間場を読み出す |

### 前提条件

- `extract_modes` は `dt > 0` の等間隔サンプリング時系列を前提とする
- 信号長が非常に短い場合（`len < 4`）や、実質ゼロ信号では空結果を返す
- ピーク検出は最大ピークに対する相対しきい値で行う
- 減衰はスペクトル幅から推定されるため、校正済み物理モデルではなく観測近似値として扱う
- `extract_spatial_modes` は与えられたスナップショット群から形状振幅を復元し、現在は平坦化した `Vec<f64>` を返す
- `ModalSet` は `ShapeLayout` に対する形状長を検証し、無効な周波数、weight、非有限の任意値を拒否する
- `ModalSet::normalized_shapes_l2()` は形状ベクトルだけを正規化し、`weight` は変更しない。L2 ノルムが 0 または非有限の形状はエラーとする
- `ModalSet::component()` / `ModalSet::components()` は `ShapeLayout` を使って配列間隔を決める。範囲外の成分インデックスはエラー、`スカラーレイアウト` では `component(0)` が自然に動く
- FFT の実装は `neco-stft` ベースの非公開な薄い層の内側に閉じ込めており、公開 API にバックエンド crate の型は現れない。内部のスペクトラム補助関数では `neco-complex::Complex` を使う
- `DampedModalSet` は外部で与える減衰係数を受ける拡張境界。コアの `ModalRecord` に `gamma` は持たず、ソルバー名のような利用側固有メタ情報は別管理
- 線形応答と振動子補助は `DampedModalSet` を前提。`gamma` は引き続きコア `ModalSet` には入れない
- `SourceExcitation` は局所的な数値制約だけを検証する。ノード番号の範囲検証は、実際にモード形状を読む補助関数側で行う
- `generate_ir_multi()` と `OscillatorBank::new_multi()` は線形の複数励起入口。非線形テーブル構築と非線形振動子ラッパーは別カテゴリで扱う

## ライセンス

MIT
