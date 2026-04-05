# neco-tui

[English](README.md)

外部依存ゼロの最小 ANSI ターミナルヘルパーです。SGR エスケープシーケンスによるテキスト装飾を提供します。

## 使い方

### スタイル付きテキスト

```rust
use neco_tui::{style, Color};

// 前景色
println!("{}", style("ok").fg(Color::Green));

// 太字 + 色
println!("{}", style("error").fg(Color::Red).bold());

// 暗い表示
println!("{}", style("hint").dim());
```

### 複数スタイルの合成

`style()` が返すビルダーはメソッドチェーンで装飾を追加できます。`Display` 実装により `println!` や `format!` で直接使えます。

```rust
use neco_tui::{style, Color, Style};

let prompt = format!(
    "{}@{} > ",
    style("host").fg(Color::Blue),
    style("user").fg(Color::Cyan).bold(),
);
```

## API

| 項目 | 説明 |
|------|------|
| `style(text)` | スタイルビルダーを返す |
| `Styled::fg(color)` | 前景色を設定 |
| `Styled::bold()` | 太字 |
| `Styled::dim()` | 暗い表示 |
| `Color` | 標準 8 色 (`Black`, `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`) |
| `Style::RESET` | SGR リセットシーケンス (`\x1b[0m`) |

## License

MIT
