# neco-brep

[English](README.md)

CSG、プロファイル駆動の立体生成、メッシュ出力を扱う解析的 B-Rep モデリングライブラリです。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## モデリングフロー

中核になるのは `Shell` です。頂点、エッジ曲線、面曲面から境界表現を組み立てます。面は `Plane`、`Cylinder`、`Cone`、`Sphere`、`Ellipsoid`、`Torus` のような解析曲面として持つことも、回転、スイープ、ロフト用に NURBS ベースのまま持つこともできます。

プリミティブ生成とプロファイル駆動生成の両方に対応し、直方体、円柱、円錐、球、楕円体、トーラスを直接作れ、`neco-nurbs` のプロファイルを `shell_from_extrude`、`shell_from_revolve`、`shell_from_sweep`、`shell_from_loft` で立体化できます。

`shell_from_sweep` はスパインを Bezier 分解済み制御点列で受け取り、`shell_from_loft` は `&[LoftSection]` と `LoftMode` を受け取ります。ロフト対象の各 section は対応する Bezier span 数が一致している必要があります。

## ブール演算と出力

`boolean_2d_all` と `boolean_3d` は結果を B-Rep のまま返し、テセレーション直前まで解析表現を保持します。`Shell::tessellate` で三角形化し、生成メッシュは `write_stl_binary` または `write_stl_ascii` で出力します。

一般的な立体表現は、標準的な解析プリミティブと通常のプロファイル駆動生成で安定しています。`shell_from_extrude` と `shell_from_revolve` は比較的安定していますが、より複雑な loft / sweep ルートはまだ詰めが残っています。

2D ブール演算の主結果型は `RegionSet` で、空結果、単一領域、複数の分離領域を表現できます。従来の `boolean_2d` は、単一領域だけを受ける呼び出し側向けの互換ヘルパです。

3D ブール演算では低次元の接触を非交差として扱います。点接触、線接触、体積を持たない接触では、`Intersect` は空シェルを返し、`Subtract` は被減数をそのまま返します。`Union` は引き続き単一の連結シェル結果を前提にします。

3D boolean は experimental 扱いです。結果の完全性はまだ保証しておらず、tessellation / 描画経路にも既知のバグが残っています。評価用途、条件を絞ったワークフロー、段階的な検証向けと考えてください。完全に信頼できる production 向け boolean パイプラインとしてはまだ位置付けていません。

## 使い方

### プリミティブのブール演算とテセレーション

```rust
use neco_brep::{
    boolean_3d, shell_from_box, shell_from_cylinder, BooleanOp,
};
use neco_brep::stl::write_stl_binary;

let a = shell_from_box(2.0, 2.0, 2.0);
let b = shell_from_cylinder(0.4, None, 2.0);

let result = boolean_3d(&a, &b, BooleanOp::Subtract)?;
let mesh = result.tessellate(24)?;

let mut bytes = Vec::new();
write_stl_binary(&mesh, &mut bytes)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### NURBS プロファイル押し出し

```rust
use neco_brep::shell_from_extrude;
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

let profile = NurbsRegion {
    outer: vec![NurbsCurve2D::circle([0.0, 0.0], 1.0)],
    holes: vec![],
};

let shell = shell_from_extrude(&profile, [0.0, 0.0, 1.0], 2.0)?;
# let _ = shell;
# Ok::<(), String>(())
```

## API

| 項目 | 説明 |
|------|-------------|
| `Shell` | 頂点、エッジ、面からなる境界表現 |
| `Surface` | 解析曲面または NURBS ベースの面形状 |
| `Curve3D` | 3D エッジ曲線型 |
| `shell_from_box` / `shell_from_cylinder` / `shell_from_cone` / `shell_from_sphere` / `shell_from_ellipsoid` / `shell_from_torus` | プリミティブ立体の生成 |
| `shell_from_extrude` / `shell_from_revolve` / `shell_from_sweep` / `shell_from_loft` | プロファイル駆動の立体生成 |
| `boolean_2d_all` / `boolean_3d` | 領域集合とシェルのブール演算 |
| `boolean_2d` | 単一領域結果のときだけ成功する互換ヘルパ |
| `RegionSet` | 0 個、1 個、または複数個の 2D ブール結果領域 |
| `BooleanOp` | `Union`, `Subtract`, `Intersect` |
| `Shell::tessellate(density)` | シェルを三角形メッシュへ変換する |
| `TriMesh` | 描画・出力用の三角形メッシュ |
| `stl::write_stl_binary` / `stl::write_stl_ascii` | テセレーション結果を STL 出力する |

## ライセンス

MIT
