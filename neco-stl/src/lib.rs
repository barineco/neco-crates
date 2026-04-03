//! STL file parser and writer with vertex deduplication.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Triangle surface mesh parsed from STL.
#[derive(Debug, Clone)]
pub struct TriSurface {
    pub nodes: Vec<[f64; 3]>,
    pub triangles: Vec<[usize; 3]>,
}

fn detect_ascii(data: &[u8]) -> bool {
    if !data.starts_with(b"solid") {
        return false;
    }
    if data.len() > 5 && !data[5].is_ascii_whitespace() {
        return false;
    }
    if data.len() >= 84 {
        let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        let expected = 84 + 50 * num_triangles;
        if expected == data.len() {
            return false;
        }
    }
    true
}

fn parse_stl_binary(data: &[u8]) -> Result<Vec<[[f32; 3]; 3]>, String> {
    if data.len() < 84 {
        return Err(
            "binary STL is too short (84 bytes required for header + triangle count)".into(),
        );
    }
    let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
    let expected = 84 + 50 * num_triangles;
    if data.len() < expected {
        return Err(format!(
            "binary STL data is truncated (expected {expected} bytes, got {})",
            data.len()
        ));
    }

    let mut triangles = Vec::with_capacity(num_triangles);
    for i in 0..num_triangles {
        let base = 84 + 50 * i;
        let mut verts = [[0.0_f32; 3]; 3];
        for (v, vert) in verts.iter_mut().enumerate() {
            let vbase = base + 12 + 12 * v;
            for (c, coord) in vert.iter_mut().enumerate() {
                let offset = vbase + 4 * c;
                *coord = f32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
            }
        }
        triangles.push(verts);
    }
    Ok(triangles)
}

fn parse_stl_ascii(data: &[u8]) -> Result<Vec<[[f32; 3]; 3]>, String> {
    let text = std::str::from_utf8(data)
        .map_err(|e| format!("failed to decode ASCII STL as UTF-8: {e}"))?;

    let mut triangles = Vec::new();
    let mut current_verts = Vec::new();
    let mut in_facet = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("facet ") {
            in_facet = true;
            current_verts.clear();
        } else if trimmed == "endfacet" {
            if !in_facet {
                return Err("encountered endfacet outside facet block".into());
            }
            if current_verts.len() != 3 {
                return Err(format!(
                    "facet must contain exactly 3 vertices, got {}",
                    current_verts.len()
                ));
            }
            triangles.push([current_verts[0], current_verts[1], current_verts[2]]);
            in_facet = false;
        } else if trimmed.starts_with("vertex ") && in_facet {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() != 4 {
                return Err(format!("invalid vertex line: {trimmed}"));
            }
            let x = parts[1]
                .parse()
                .map_err(|_| format!("failed to parse vertex coordinate: {}", parts[1]))?;
            let y = parts[2]
                .parse()
                .map_err(|_| format!("failed to parse vertex coordinate: {}", parts[2]))?;
            let z = parts[3]
                .parse()
                .map_err(|_| format!("failed to parse vertex coordinate: {}", parts[3]))?;
            current_verts.push([x, y, z]);
        }
    }

    Ok(triangles)
}

/// Parse STL bytes into a triangle surface mesh.
pub fn parse_stl(data: &[u8]) -> Result<TriSurface, String> {
    let raw_triangles = if detect_ascii(data) {
        parse_stl_ascii(data)?
    } else {
        parse_stl_binary(data)?
    };

    let mut nodes = Vec::new();
    let mut triangles = Vec::new();
    let mut vertex_map: HashMap<[u64; 3], usize> = HashMap::new();

    let quantize = |v: f32| -> u64 { ((v as f64) * 1e6).round().to_bits() };

    for tri in &raw_triangles {
        let mut indices = [0usize; 3];
        for (i, v) in tri.iter().enumerate() {
            let key = [quantize(v[0]), quantize(v[1]), quantize(v[2])];
            let idx = if let Some(&existing) = vertex_map.get(&key) {
                existing
            } else {
                let idx = nodes.len();
                nodes.push([v[0] as f64, v[1] as f64, v[2] as f64]);
                vertex_map.insert(key, idx);
                idx
            };
            indices[i] = idx;
        }
        if indices[0] != indices[1] && indices[1] != indices[2] && indices[2] != indices[0] {
            triangles.push(indices);
        }
    }

    if triangles.is_empty() {
        return Err("STL file contains no valid triangles".into());
    }

    Ok(TriSurface { nodes, triangles })
}

fn write_f32_triple(writer: &mut dyn Write, v: [f64; 3]) -> std::io::Result<()> {
    for &c in &v {
        writer.write_all(&(c as f32).to_le_bytes())?;
    }
    Ok(())
}

fn checked_triangle_count(len: usize) -> std::io::Result<u32> {
    u32::try_from(len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "triangle count exceeds u32::MAX for STL binary format",
        )
    })
}

/// Write a binary STL file.
pub fn write_stl_binary(
    nodes: &[[f64; 3]],
    triangles: &[[usize; 3]],
    path: &Path,
) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&[0u8; 80])?;
    let n_tris = checked_triangle_count(triangles.len())?;
    writer.write_all(&n_tris.to_le_bytes())?;

    for tri in triangles {
        let v0 = nodes[tri[0]];
        let v1 = nodes[tri[1]];
        let v2 = nodes[tri[2]];
        let normal = triangle_normal(v0, v1, v2);

        write_f32_triple(&mut writer, normal)?;
        write_f32_triple(&mut writer, v0)?;
        write_f32_triple(&mut writer, v1)?;
        write_f32_triple(&mut writer, v2)?;
        writer.write_all(&0u16.to_le_bytes())?;
    }

    writer.flush()
}

/// Write an ASCII STL file.
pub fn write_stl_ascii(
    nodes: &[[f64; 3]],
    triangles: &[[usize; 3]],
    path: &Path,
) -> std::io::Result<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writeln!(writer, "solid mesh")?;

    for tri in triangles {
        let v0 = nodes[tri[0]];
        let v1 = nodes[tri[1]];
        let v2 = nodes[tri[2]];
        let normal = triangle_normal(v0, v1, v2);

        writeln!(
            writer,
            "  facet normal {} {} {}",
            normal[0], normal[1], normal[2]
        )?;
        writeln!(writer, "    outer loop")?;
        writeln!(writer, "      vertex {} {} {}", v0[0], v0[1], v0[2])?;
        writeln!(writer, "      vertex {} {} {}", v1[0], v1[1], v1[2])?;
        writeln!(writer, "      vertex {} {} {}", v2[0], v2[1], v2[2])?;
        writeln!(writer, "    endloop")?;
        writeln!(writer, "  endfacet")?;
    }

    writeln!(writer, "endsolid mesh")?;
    writer.flush()
}

#[cfg(test)]
mod write_tests {
    use super::*;

    #[test]
    fn binary_writer_rejects_triangle_count_above_u32() {
        let error = checked_triangle_count(usize::MAX).expect_err("usize::MAX exceeds u32");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}

fn triangle_normal(v0: [f64; 3], v1: [f64; 3], v2: [f64; 3]) -> [f64; 3] {
    let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    let cx = e1[1] * e2[2] - e1[2] * e2[1];
    let cy = e1[2] * e2[0] - e1[0] * e2[2];
    let cz = e1[0] * e2[1] - e1[1] * e2[0];
    let len = (cx * cx + cy * cy + cz * cz).sqrt();
    if len < 1e-15 {
        [0.0, 0.0, 0.0]
    } else {
        [cx / len, cy / len, cz / len]
    }
}

impl TriSurface {
    /// Compute per-face normals.
    pub fn face_normals(&self) -> Vec<[f64; 3]> {
        self.triangles
            .iter()
            .map(|tri| triangle_normal(self.nodes[tri[0]], self.nodes[tri[1]], self.nodes[tri[2]]))
            .collect()
    }

    /// Extract feature edges using the angle between adjacent face normals.
    pub fn feature_edges(&self, angle_threshold_deg: f64) -> Vec<[usize; 2]> {
        let normals = self.face_normals();
        let cos_threshold = angle_threshold_deg.to_radians().cos();
        let mut edge_faces: HashMap<(usize, usize), Vec<usize>> = HashMap::new();

        for (fi, tri) in self.triangles.iter().enumerate() {
            for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let key = if a < b { (a, b) } else { (b, a) };
                edge_faces.entry(key).or_default().push(fi);
            }
        }

        let mut result = Vec::new();
        for (&(a, b), faces) in &edge_faces {
            let is_feature = if faces.len() == 1 {
                true
            } else if faces.len() == 2 {
                let n0 = normals[faces[0]];
                let n1 = normals[faces[1]];
                let dot = n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2];
                dot < cos_threshold
            } else {
                true
            };
            if is_feature {
                result.push([a, b]);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_binary_stl(triangles: &[[[f32; 3]; 3]]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0u8; 80]);
        let count = u32::try_from(triangles.len()).expect("triangle count exceeds u32");
        buf.extend_from_slice(&count.to_le_bytes());
        for tri in triangles {
            buf.extend_from_slice(&[0u8; 12]);
            for v in tri {
                for &c in v {
                    buf.extend_from_slice(&c.to_le_bytes());
                }
            }
            buf.extend_from_slice(&[0u8; 2]);
        }
        buf
    }

    #[test]
    fn parse_ascii_stl() {
        let ascii = b"solid test
facet normal 0 0 1
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 0 1 0
  endloop
endfacet
facet normal 0 0 1
  outer loop
    vertex 1 0 0
    vertex 1 1 0
    vertex 0 1 0
  endloop
endfacet
endsolid test";

        let surface = parse_stl(ascii).unwrap();
        assert_eq!(surface.nodes.len(), 4);
        assert_eq!(surface.triangles.len(), 2);
    }

    #[test]
    fn degenerate_triangle_filtered() {
        let ascii = b"solid test
facet normal 0 0 1
  outer loop
    vertex 0 0 0
    vertex 0 0 0
    vertex 0 1 0
  endloop
endfacet
facet normal 0 0 1
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 0 1 0
  endloop
endfacet
endsolid test";

        let surface = parse_stl(ascii).unwrap();
        assert_eq!(surface.triangles.len(), 1);
    }

    #[test]
    fn parse_binary_stl_single_triangle() {
        let tri = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let data = make_binary_stl(&[[tri[0], tri[1], tri[2]]]);
        let surface = parse_stl(&data).unwrap();
        assert_eq!(surface.nodes.len(), 3);
        assert_eq!(surface.triangles.len(), 1);
    }

    #[test]
    fn parse_binary_stl_multiple_triangles() {
        let tris = [
            [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [[1.0f32, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        ];
        let data = make_binary_stl(&tris);
        let surface = parse_stl(&data).unwrap();
        assert_eq!(surface.nodes.len(), 4);
        assert_eq!(surface.triangles.len(), 2);
    }

    #[test]
    fn binary_stl_truncated_error() {
        let data = vec![0u8; 50];
        let result = parse_stl(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too short"));
    }

    #[test]
    fn binary_stl_data_shortage_error() {
        let mut data = vec![0u8; 84];
        data[80] = 1;
        let result = parse_stl(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("truncated"));
    }

    #[test]
    fn binary_stl_zero_triangles_error() {
        let data = make_binary_stl(&[]);
        let result = parse_stl(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no valid triangles"));
    }

    #[test]
    fn detect_ascii_vs_binary() {
        let ascii = b"solid test\nfacet normal 0 0 1\nendsolid test";
        assert!(detect_ascii(ascii));

        let binary = vec![0u8; 84];
        assert!(!detect_ascii(&binary));

        let mut tricky = vec![0u8; 84];
        tricky[..5].copy_from_slice(b"solid");
        tricky[5] = b' ';
        assert!(!detect_ascii(&tricky));
    }

    #[test]
    fn test_face_normals_basic() {
        let surface = TriSurface {
            nodes: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            triangles: vec![[0, 1, 2]],
        };
        let normals = surface.face_normals();
        assert_eq!(normals.len(), 1);
        assert!(normals[0][0].abs() < 1e-10);
        assert!(normals[0][1].abs() < 1e-10);
        assert!((normals[0][2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_face_normals_degenerate() {
        let surface = TriSurface {
            nodes: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            triangles: vec![[0, 1, 2]],
        };
        let normals = surface.face_normals();
        assert_eq!(normals.len(), 1);
        assert!(normals[0][0].abs() < 1e-10);
        assert!(normals[0][1].abs() < 1e-10);
        assert!(normals[0][2].abs() < 1e-10);
    }

    #[test]
    fn test_feature_edges_cube() {
        let nodes = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 6, 2],
            [3, 7, 6],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ];
        let surface = TriSurface { nodes, triangles };
        let edges = surface.feature_edges(30.0);
        assert_eq!(edges.len(), 12);
    }

    #[test]
    fn write_stl_files() {
        let dir = std::env::temp_dir();
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let binary_path = dir.join(format!("neco-stl-{suffix}.bin.stl"));
        let ascii_path = dir.join(format!("neco-stl-{suffix}.ascii.stl"));

        let nodes = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let triangles = [[0usize, 1, 2]];
        write_stl_binary(&nodes, &triangles, &binary_path).unwrap();
        write_stl_ascii(&nodes, &triangles, &ascii_path).unwrap();

        let binary = std::fs::read(&binary_path).unwrap();
        let ascii = std::fs::read_to_string(&ascii_path).unwrap();
        assert_eq!(binary.len(), 134);
        assert!(ascii.starts_with("solid mesh"));
        assert!(ascii.contains("facet normal"));

        let _ = std::fs::remove_file(binary_path);
        let _ = std::fs::remove_file(ascii_path);
    }
}
