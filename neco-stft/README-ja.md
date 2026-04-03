# neco-stft

[English](README.md)

バックエンド非依存の実数 FFT ファサード、窓関数、STFT / ISTFT をまとめた crate です。

FFT バックエンドの詳細は小さな公開境界の内側で閉じ、内部実装を差し替えても利用側 API は変えず、複素スペクトルの公開境界にはバックエンド付属型ではなく `neco-complex::Complex` を使います。

## API

| 項目 | 説明 |
|------|------|
| `FftError` | FFT のバッファ長不一致エラー |
| `RealToComplex<T>` | 実数から複素数への変換仕様 |
| `ComplexToReal<T>` | 複素数から実数への変換仕様 |
| `FftPlanner<T>` | 順変換／逆変換の計画を担う planner trait |
| `RustFftPlannerF32`, `RustFftPlannerF64` | 現在の既定計画器実装。2 冪長は crate 内 radix-2、非 2 冪長は crate 内一般長 FFT |
| `DspFloat` | スレッドローカル計画器を持つ `f32` / `f64` 向け数値 trait |
| `hann(n)` | Hann 窓関数 |
| `kaiser_bessel_derived(n, alpha)` | KBD 窓関数 |
| `StftProcessor` | WOLA 正規化付き STFT / ISTFT 処理機 |
| `SpectrumFrame<T>` | 正周波数側の複素スペクトルフレーム |

## 前提条件

- 公開 FFT ファサードは実数対複素・複素対実数変換のみを扱う
- 逆変換は正規化なし。必要な `1 / N` は呼び出し側で掛ける
- `StftProcessor` は固定 hop 前提の重み付き重なり加算（WOLA）正規化を内部で行う
- バックエンド固有の具体的変換型は公開 API に出しません
- 安定な公開境界の中心は planner trait 側。`RustFftPlannerF32` / `RustFftPlannerF64` は現在の既定実装名で、公開境界そのものではない
- 2 冪長長には crate 内 radix-2 バックエンドを使い、非 2 冪長長には同じファサード配下の一般長 FFT バックエンドを使う
- 公開スペクトル境界には `neco-complex::Complex` を使い、バックエンド固有の複素バッファはファサード内側に閉じる

## ライセンス

MIT
