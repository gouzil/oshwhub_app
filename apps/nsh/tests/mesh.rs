#[path = "../src/viewer/mesh.rs"]
mod mesh;

use mesh::{MeshCounts, validate_instances, validate_mesh};
use oshwhub_core::pcb::{Bounds3, Mesh};

const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

fn triangle() -> Mesh {
    Mesh {
        vertices: vec![
            0., 0., 0., 0., 0., 1., 1., 0., 0., 2., 0., 0., 0., 0., 1., 1., 0., 0., 0., 1., 0., 0.,
            0., 1., 1., 0., 0.,
        ],
        indices: vec![0, 1, 2],
        bounds: Bounds3 {
            min: [0., 0., 0.],
            max: [2., 1., 0.],
        },
    }
}

#[test]
fn raw_triangle_has_checked_draw_counts_and_exact_buffer_sizes() {
    let mesh = triangle();
    assert_eq!(
        validate_mesh(&mesh, 108).unwrap(),
        MeshCounts {
            vertices: 3,
            indices: 3,
            vertex_bytes: 108,
            index_bytes: 12,
        }
    );
}

#[test]
fn malformed_vertices_bounds_and_triangles_are_rejected_before_upload() {
    let mut empty = triangle();
    empty.vertices.clear();
    assert!(validate_mesh(&empty, 1024).is_err());
    let mut stride = triangle();
    stride.vertices.pop();
    assert!(validate_mesh(&stride, 1024).is_err());
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut nonfinite = triangle();
        nonfinite.vertices[8] = bad;
        assert!(validate_mesh(&nonfinite, 1024).is_err());
    }
    for indices in [vec![], vec![0, 1], vec![0, 1, 3], vec![u32::MAX, 1, 2]] {
        let mesh = Mesh {
            indices,
            ..triangle()
        };
        assert!(validate_mesh(&mesh, 1024).is_err());
    }
    for bounds in [
        Bounds3 {
            min: [f64::NAN, 0., 0.],
            max: [2., 1., 0.],
        },
        Bounds3 {
            min: [0., 0., 0.],
            max: [f64::INFINITY, 1., 0.],
        },
        Bounds3 {
            min: [3., 0., 0.],
            max: [2., 1., 0.],
        },
    ] {
        let mesh = Mesh {
            bounds,
            ..triangle()
        };
        assert!(validate_mesh(&mesh, 1024).is_err());
    }
}

#[test]
fn vertex_and_index_buffers_each_respect_the_device_byte_limit() {
    assert!(validate_mesh(&triangle(), 107).is_err());
    let mesh = Mesh {
        indices: [0, 1, 2].repeat(12),
        ..triangle()
    };
    assert!(validate_mesh(&mesh, 143).is_err());
    assert_eq!(validate_mesh(&mesh, 144).unwrap().index_bytes, 144);
}

#[test]
fn instance_matrices_are_finite_and_fit_the_device_byte_limit() {
    assert_eq!(validate_instances([&IDENTITY], 64).unwrap(), 1);
    assert_eq!(validate_instances([&IDENTITY, &IDENTITY], 128).unwrap(), 2);
    assert_eq!(validate_instances(std::iter::empty(), 0).unwrap(), 0);
    assert!(validate_instances([&IDENTITY], 63).is_err());
    assert!(validate_instances([&IDENTITY, &IDENTITY], 127).is_err());
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut matrix = IDENTITY;
        matrix[12] = bad;
        assert!(validate_instances([&matrix], 64).is_err());
    }
}
