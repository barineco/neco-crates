# neco-minphase

[English](README.md)

最小位相スペクトルとインパルスカーネルの OLA 畳み込みをまとめた crate です。

cepstrum と畳み込みの純粋なコア部分を切り出し、再利用しやすい形に絞って、複素スペクトルの公開境界には共有の `neco-complex::Complex` を使う

## API

| 項目 | 説明 |
|------|------|
| `compute_min_phase_spectrum(gain_curve, fft_size)` | 振幅応答曲線から最小位相複素スペクトルを作る |
| `compute_min_phase_ir(gain_curve, fft_size)` | 最小位相インパルス応答を作る |
| `convolve_ola(input, ir)` | FFT の重なり加算法で畳み込みを実行し、入力長に切り詰めて返す |
| `compute_blend_curve(transient_map, lookahead, smooth, threshold)` | `[0, 1]` の過渡適応ブレンド曲線を作る |

## 前提条件

- `gain_curve` の長さは `fft_size / 2 + 1` でなければなりません
- `fft_size` は 2 冪長だけでなく、`neco-stft` の公開 FFT ファサードが扱う一般長も使える
- 最小位相カーネルは振幅を保ちつつ、インパルスの前半へエネルギーを寄せる
- `convolve_ola` の返り値は入力長に切り詰められる
- 上位 EQ 組立て API は別扱いとして分離

## ライセンス

MIT
