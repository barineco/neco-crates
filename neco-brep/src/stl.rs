//! STL file export (binary / ASCII).

use crate::tessellate::TriMesh;
use crate::vec3;
use std::io::Write;

fn checked_triangle_count(len: usize) -> std::io::Result<u32> {
    u32::try_from(len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "triangle count exceeds u32::MAX for STL binary format",
        )
    })
}

/// Write binary STL.
pub fn write_stl_binary(mesh: &TriMesh, writer: &mut dyn Write) -> std::io::Result<()> {
    let header = [0u8; 80];
    writer.write_all(&header)?;

    let n_tris = checked_triangle_count(mesh.triangles.len())?;
    writer.write_all(&n_tris.to_le_bytes())?;

    for tri in &mesh.triangles {
        let v0 = mesh.vertices[tri[0]];
        let v1 = mesh.vertices[tri[1]];
        let v2 = mesh.vertices[tri[2]];

        let normal = vec3::normalized(vec3::cross(vec3::sub(v1, v0), vec3::sub(v2, v0)));

        write_f32_triple(writer, normal)?;
        write_f32_triple(writer, v0)?;
        write_f32_triple(writer, v1)?;
        write_f32_triple(writer, v2)?;
        writer.write_all(&0u16.to_le_bytes())?;
    }

    Ok(())
}

/// Write ASCII STL.
pub fn write_stl_ascii(mesh: &TriMesh, writer: &mut dyn Write) -> std::io::Result<()> {
    writeln!(writer, "solid mesh")?;

    for tri in &mesh.triangles {
        let v0 = mesh.vertices[tri[0]];
        let v1 = mesh.vertices[tri[1]];
        let v2 = mesh.vertices[tri[2]];

        let normal = vec3::normalized(vec3::cross(vec3::sub(v1, v0), vec3::sub(v2, v0)));

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
    Ok(())
}

/// Write f64x3 as f32 little-endian.
fn write_f32_triple(writer: &mut dyn Write, v: [f64; 3]) -> std::io::Result<()> {
    for &c in &v {
        writer.write_all(&(c as f32).to_le_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_single_triangle_mesh() -> TriMesh {
        TriMesh {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            triangles: vec![[0, 1, 2]],
        }
    }

    #[test]
    fn binary_stl_size() {
        let mesh = make_single_triangle_mesh();
        let mut buf = Vec::new();
        write_stl_binary(&mesh, &mut buf).expect("binary STL write failed");
        // 80 + 4 + 50 * 1 = 134
        assert_eq!(buf.len(), 134);
    }

    #[test]
    fn binary_stl_triangle_count() {
        let mesh = make_single_triangle_mesh();
        let mut buf = Vec::new();
        write_stl_binary(&mesh, &mut buf).expect("binary STL write failed");
        let count = u32::from_le_bytes([buf[80], buf[81], buf[82], buf[83]]);
        assert_eq!(count, 1);
    }

    #[test]
    fn ascii_stl_structure() {
        let mesh = make_single_triangle_mesh();
        let mut buf = Vec::new();
        write_stl_ascii(&mesh, &mut buf).expect("ASCII STL write failed");
        let text = String::from_utf8(buf).expect("UTF-8 conversion failed");
        assert!(text.starts_with("solid mesh"));
        assert!(text.contains("facet normal"));
        assert!(text.contains("outer loop"));
        assert!(text.contains("vertex"));
        assert!(text.contains("endloop"));
        assert!(text.contains("endfacet"));
        assert!(text.trim_end().ends_with("endsolid mesh"));
    }

    #[test]
    fn binary_writer_rejects_triangle_count_above_u32() {
        let error = checked_triangle_count(usize::MAX).expect_err("usize::MAX exceeds u32");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
