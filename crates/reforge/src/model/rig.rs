//! A rig's files read to what the facts ask of them: a skeleton's joints, a mesh's submeshes and
//! vertex positions, the joints a clip animates. The bytes may be a mod's, so each reader of
//! another crate runs where its panic is caught, and bytes that do not read give `None`.

use std::collections::HashSet;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ltk_anim::{AnimationAsset, RigResource};
use ltk_mesh::SkinnedMesh;
use ltk_mesh::mem::vertex::ElementName;

/// The parts of a unit a vertex position is rounded to: 1/16, as the census history rounds a
/// mesh's positions for its `rounded_id`.
pub const ROUND: f32 = 16.0;

/// A vertex position rounded to 1/[`ROUND`] unit.
pub type Rounded = (i32, i32, i32);

/// The ELF hashes a clip may bind a joint's track by: of the name as spelled and lowercased.
/// The casing rule varies with the clip's format.
pub fn joint_hashes(name: &str) -> [u32; 2] {
    let spelled = ltk_hash::elf::elf(name) as u32;
    let lower = ltk_hash::elf::elf(name.to_ascii_lowercase()) as u32;
    [spelled, lower]
}

/// A skeleton as the facts read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skeleton {
    /// The joints' names, in the file's order.
    pub joints: Vec<String>,
    /// How many joints a vertex may bind to.
    pub influences: usize,
}

impl Skeleton {
    /// The skeleton of a file's bytes. `None` when they do not read.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let read = || RigResource::from_reader(&mut Cursor::new(bytes)).ok();
        let rig = catch_unwind(AssertUnwindSafe(read)).ok()??;
        Some(Self {
            joints: rig.joints().iter().map(|j| j.name().to_string()).collect(),
            influences: rig.influences().len(),
        })
    }

    /// The hashes a clip's track may name a joint of the skeleton by.
    pub fn joint_hashes(&self) -> HashSet<u32> {
        self.joints.iter().flat_map(|j| joint_hashes(j)).collect()
    }

    /// The joint a hash names, as the skeleton spells it.
    pub fn joint_named(&self, hash: u32) -> Option<&str> {
        let named = |joint: &&String| joint_hashes(joint).contains(&hash);
        self.joints.iter().find(named).map(String::as_str)
    }
}

/// A submesh: one range of a mesh, drawn with one material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submesh {
    /// Its name.
    pub name: String,
    /// The positions of its vertices, rounded.
    pub positions: HashSet<Rounded>,
}

/// A skinned mesh as the facts read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshShape {
    /// The submeshes, in the file's order.
    pub submeshes: Vec<Submesh>,
    /// The positions of every submesh's vertices, rounded.
    pub all: HashSet<Rounded>,
    /// The highest joint index a vertex binds to. `None` when the mesh holds no blend indices.
    pub max_blend: Option<u8>,
}

fn round(v: glam::Vec3) -> Rounded {
    let part = |x: f32| (x * ROUND).round() as i32;
    (part(v.x), part(v.y), part(v.z))
}

impl MeshShape {
    /// The shape of a mesh file's bytes. `None` when they do not read, or hold no positions.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        catch_unwind(AssertUnwindSafe(|| Self::read(bytes))).ok()?
    }

    fn read(bytes: &[u8]) -> Option<Self> {
        let mesh = SkinnedMesh::from_reader(&mut Cursor::new(bytes)).ok()?;
        let buffer = mesh.vertex_buffer();
        let positions = buffer.accessor::<glam::Vec3>(ElementName::Position)?;
        let blend = buffer.accessor::<[u8; 4]>(ElementName::BlendIndex);
        let mut all = HashSet::new();
        let mut submeshes = Vec::new();
        for range in mesh.ranges() {
            let start = range.start_vertex.max(0) as usize;
            let end = (start + range.vertex_count.max(0) as usize).min(positions.len());
            let set: HashSet<Rounded> = (start..end).map(|i| round(positions.get(i))).collect();
            all.extend(set.iter().copied());
            submeshes.push(Submesh {
                name: range.material.clone(),
                positions: set,
            });
        }
        let max_blend = blend.map(|b| (0..b.len()).flat_map(|i| b.get(i)).max().unwrap_or(0));
        Some(Self {
            submeshes,
            all,
            max_blend,
        })
    }
}

/// The hashes of the joints a clip animates. `None` when the bytes do not read.
pub fn clip_joints(bytes: &[u8]) -> Option<Vec<u32>> {
    let read = || AnimationAsset::from_reader(&mut Cursor::new(bytes)).ok();
    let asset = catch_unwind(AssertUnwindSafe(read)).ok()??;
    Some(match asset {
        AnimationAsset::Compressed(clip) => clip.joints().to_vec(),
        AnimationAsset::Uncompressed(clip) => clip.joints().into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_that_are_no_rig_file_read_as_nothing() {
        assert_eq!(Skeleton::parse(b"not a skeleton"), None);
        assert_eq!(MeshShape::parse(b"not a mesh"), None);
        assert_eq!(clip_joints(b"not a clip"), None);
        assert_eq!(Skeleton::parse(&[]), None);
    }

    #[test]
    fn a_joint_is_named_by_both_spellings() {
        let skeleton = Skeleton {
            joints: vec!["Root".into(), "L_Hand".into()],
            influences: 2,
        };
        let [spelled, lower] = joint_hashes("L_Hand");
        assert_ne!(spelled, lower);
        assert_eq!(skeleton.joint_named(lower), Some("L_Hand"));
        assert_eq!(skeleton.joint_hashes().len(), 4);
    }
}
