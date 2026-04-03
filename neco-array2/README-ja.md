# neco-array2

[English](README.md)

格子系 crate 向けの軽量行優先 2D 配列基盤です。

マスク、フィールド配列、復元点データ用の平坦ストレージに絞った `Array2<T>` を持ちます。
`neco-gridfield` や `neco-contact` のような格子系 crate で共有するための型で、配列演算の範囲は必要最小限に限定しています。

## API

| 項目 | 説明 |
|------|------|
| `Array2::from_shape_vec((nrows, ncols), data)` | 行優先の所有ストレージから形状検証付きで構築 |
| `Array2::from_elem((nrows, ncols), value)` | 単一値で埋めた配列を構築 |
| `Array2::zeros((nrows, ncols))` | `T: Default` のときゼロ初期化配列を構築 |
| `Array2::dim()` | `(nrows, ncols)` を返す |
| `Array2::shape()` | `[nrows, ncols]` を返す |
| `Array2::as_slice()` | 内部の行優先ストレージを公開する |
| `Array2::iter()` / `iter_mut()` | 行優先ストレージを走査する |
| `array[[row, col]]` | 1 セルを読み書きする |

## 前提条件

- ストレージ順序は行優先
- API は意図的に狭くし、線形代数、スライス、ブロードキャスト補助を持たない
- この型は内部の格子系 crate 間で共有するための配列基盤で、配列演算は必要最小限に限定
- 行優先の復元点データを追加依存なしで行き来できるようシリアライズ機能を持つ

## ライセンス

MIT
