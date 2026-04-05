# neco-tui

[日本語](README-ja.md)

Zero-dependency minimal ANSI terminal helpers. Provides text styling via SGR escape sequences.

## Usage

### Styled text

```rust
use neco_tui::{style, Color};

// Foreground color
println!("{}", style("ok").fg(Color::Green));

// Bold + color
println!("{}", style("error").fg(Color::Red).bold());

// Dim display
println!("{}", style("hint").dim());
```

### Composing styles

The builder returned by `style()` supports method chaining. It implements `Display` for use with `println!` and `format!`.

```rust
use neco_tui::{style, Color, Style};

let prompt = format!(
    "{}@{} > ",
    style("host").fg(Color::Blue),
    style("user").fg(Color::Cyan).bold(),
);
```

## API

| Item | Description |
|------|-------------|
| `style(text)` | Returns a style builder |
| `Styled::fg(color)` | Set foreground color |
| `Styled::bold()` | Bold text |
| `Styled::dim()` | Dim text |
| `Color` | Standard 8 colors (`Black`, `Red`, `Green`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `White`) |
| `Style::RESET` | SGR reset sequence (`\x1b[0m`) |

## License

MIT
