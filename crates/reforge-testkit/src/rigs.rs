//! Rig files for tests: a skinned mesh, a skeleton and a clip that the readers read, built
//! from the few things a test states of them.

use std::collections::HashMap;
use std::io::Cursor;

use glam::{Quat, Vec3};
use ltk_anim::asset::UncompressedFrame;
use ltk_anim::{JointBuilder, RigBuilder, Uncompressed};
use ltk_mesh::mem::vertex::{VertexBufferDescription, VertexBufferUsage, VertexElement};
use ltk_mesh::mem::{IndexBuffer, VertexBuffer};
use ltk_mesh::{SkinnedMesh, SkinnedMeshRange};

/// A skinned mesh of these submeshes, each a name and the positions of its vertices. Every
/// vertex binds to the joint at index `joint`.
///
/// # Panics
///
/// When `ltk_mesh` does not write the mesh.
pub fn mesh(submeshes: &[(&str, &[[f32; 3]])], joint: u8) -> Vec<u8> {
    let elements = vec![
        VertexElement::POSITION,
        VertexElement::BLEND_INDEX,
        VertexElement::BLEND_WEIGHT,
        VertexElement::NORMAL,
        VertexElement::TEXCOORD_0,
    ];
    let description = VertexBufferDescription::new(VertexBufferUsage::Static, elements);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut ranges = Vec::new();
    let (mut vertex_at, mut index_at) = (0i32, 0i32);
    for (name, positions) in submeshes {
        for position in *positions {
            for part in position {
                vertices.extend(part.to_le_bytes());
            }
            vertices.extend([joint, 0, 0, 0]);
            for weight in [1.0f32, 0.0, 0.0, 0.0] {
                vertices.extend(weight.to_le_bytes());
            }
            // The normal, then the texture coordinate.
            for part in [0.0f32, 1.0, 0.0, 0.0, 0.0] {
                vertices.extend(part.to_le_bytes());
            }
        }
        // One triangle over the first vertices: the readers ask nothing of the faces.
        for index in [0u16, 0, 0] {
            indices.extend(index.to_le_bytes());
        }
        let count = positions.len() as i32;
        ranges.push(SkinnedMeshRange::new(*name, vertex_at, count, index_at, 3));
        vertex_at += count;
        index_at += 3;
    }
    let mesh = SkinnedMesh::new(
        ranges,
        VertexBuffer::new(description, vertices),
        IndexBuffer::<u16>::new(indices),
    );
    let mut out = Vec::new();
    let written = mesh.to_writer(&mut out);
    written
        .unwrap_or_else(|error| panic!("a mesh of {} submeshes writes: {error}", submeshes.len()));
    out
}

/// A skeleton of these joints: the first is the root, the others its children, each one a
/// vertex may bind to.
///
/// # Panics
///
/// When no joint is given, or `ltk_anim` does not write the skeleton.
pub fn skeleton(joints: &[&str]) -> Vec<u8> {
    let (root, children) = joints.split_first().expect("a skeleton has a root joint");
    let joint = |name: &&str| Box::new(JointBuilder::new(*name).with_influence(true));
    let root = JointBuilder::new(*root).with_influence(true);
    let root = root.with_children(children.iter().map(joint));
    let rig = RigBuilder::new("skeleton", "skeleton")
        .with_root_joint(root)
        .build();
    let mut out = Cursor::new(Vec::new());
    let written = rig.to_writer(&mut out);
    written.unwrap_or_else(|error| panic!("a skeleton of {} joints writes: {error}", joints.len()));
    out.into_inner()
}

/// A clip of one frame with a track for each of these joints, bound by the hash of the
/// joint's lowercase name.
///
/// # Panics
///
/// When `ltk_anim` does not write the clip.
pub fn clip(joints: &[&str]) -> Vec<u8> {
    let frame = UncompressedFrame {
        translation_id: 0,
        scale_id: 1,
        rotation_id: 0,
    };
    let track = |name: &&str| {
        let hash = ltk_hash::elf::elf(name.to_ascii_lowercase()) as u32;
        (hash, vec![frame])
    };
    let tracks: HashMap<u32, Vec<UncompressedFrame>> = joints.iter().map(track).collect();
    let clip = Uncompressed::new(
        30.0,
        vec![Vec3::ZERO, Vec3::ONE],
        vec![Quat::IDENTITY],
        tracks,
    );
    let mut out = Cursor::new(Vec::new());
    let written = clip.to_writer(&mut out);
    written.unwrap_or_else(|error| panic!("a clip of {} tracks writes: {error}", joints.len()));
    out.into_inner()
}
