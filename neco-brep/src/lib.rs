//! Analytical B-Rep engine supporting analytic surfaces (Plane, Cylinder, Cone,
//! Sphere, Ellipsoid, Torus, SurfaceOfRevolution, SurfaceOfSweep) and NURBS surfaces.

pub mod bezier;
pub mod bezier_decompose;
pub mod boolean;
pub mod boolean3d;
pub mod brep;
pub mod extrude;
pub mod loft;
pub mod mesh_validate;
pub mod primitives;
pub mod radians;
pub mod revolve;
pub mod shell_view;
pub mod stl;
pub mod sweep;
pub mod tessellate;
pub mod transform;
pub mod types;
pub mod vec3;

pub use boolean::{boolean_2d, boolean_2d_all, RegionSet};
pub use boolean3d::boolean_3d;
pub use brep::{
    eval_revolution_profile, find_closest_v_on_profile, Curve3D, Edge, EdgeId, EdgeRef, Face,
    FaceId, Shell, SubFace, Surface, VertexId,
};
pub use extrude::shell_from_extrude;
pub use loft::shell_from_loft;
pub use mesh_validate::MeshValidation;
pub use primitives::{
    shell_from_box, shell_from_cone, shell_from_cylinder, shell_from_ellipsoid, shell_from_sphere,
    shell_from_torus,
};
pub use radians::Radians;
pub use revolve::shell_from_revolve;
pub use shell_view::{shell_view, FaceSample, FaceView, ShellView};
pub use sweep::shell_from_sweep;
pub use tessellate::TriMesh;
pub use transform::{apply_transform, solid_to_shell};
pub use types::{Axis, BooleanOp, LoftMode, LoftSection};
