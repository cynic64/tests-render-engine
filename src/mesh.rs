use render_engine::{RenderPass, Queue, Format};
use render_engine::mesh::{Mesh, VertexType, PrimitiveTopology, ObjectPrototype};
use render_engine::system::RenderableObject;
use render_engine::utils::{bufferize_data, load_texture};
use render_engine::pipeline_cache::PipelineSpec;
use render_engine::collection_cache::{pds_for_buffers, pds_for_images};

use nalgebra_glm::*;

use std::path::Path;
use std::sync::Arc;
use std::marker::PhantomData;

use crate::{relative_path, default_sampler};

pub fn load_obj_single(path: &Path) -> Mesh<PosTexNorm> {
    // loads the first object in an OBJ file, without materials
    let (models, _materials) = tobj::load_obj(path).expect("Couldn't load OBJ file");

    // only use first mesh
    let mesh = &models[0].mesh;
    let mut vertices: Vec<PosTexNorm> = vec![];

    for i in 0..mesh.positions.len() / 3 {
        let pos = [
            mesh.positions[i * 3],
            mesh.positions[i * 3 + 1],
            mesh.positions[i * 3 + 2],
        ];
        let normal = [
            mesh.normals[i * 3],
            mesh.normals[i * 3 + 1],
            mesh.normals[i * 3 + 2],
        ];
        let tex_coord = if mesh.texcoords.len() <= i * 2 + 1 {
            [0.0, 0.0]
        } else {
            [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1] * -1.0]
        };

        let vertex = PosTexNorm {
            position: pos,
            tex_coord,
            normal,
        };

        vertices.push(vertex);
    }

    println!("Vertices: {}", vertices.len());
    println!("Indices: {}", mesh.indices.len());

    Mesh {
        vertices,
        indices: mesh.indices.clone(),
    }
}

pub fn add_tangents(mesh: &Mesh<PosTexNorm>) -> Mesh<PosTexNormTan> {
    // use to compute tangents for a mesh with normals and texture coordinates
    let (vertices, indices) = (&mesh.vertices, &mesh.indices);

    let mut tangents: Vec<Vec3> = vec![vec3(0.0, 0.0, 0.0); vertices.len()];

    for i in 0..indices.len() / 3 {
        let face = [
            vertices[indices[i * 3] as usize],
            vertices[indices[i * 3 + 1] as usize],
            vertices[indices[i * 3 + 2] as usize],
        ];
        let (tangent, _bitangent) = tangent_bitangent_for_face(&face);
        tangents[indices[i * 3] as usize] += tangent;
        tangents[indices[i * 3 + 1] as usize] += tangent;
        tangents[indices[i * 3 + 2] as usize] += tangent;
    }

    let new_vertices: Vec<PosTexNormTan> = vertices
        .iter()
        .enumerate()
        .map(|(idx, v)| {
            let t = normalize(&tangents[idx]);

            PosTexNormTan {
                position: v.position,
                tex_coord: v.tex_coord,
                normal: v.normal,
                tangent: t.into(),
            }
        })
        .collect();

    Mesh {
        vertices: new_vertices,
        indices: indices.clone(),
    }
}

fn tangent_bitangent_for_face(face: &[PosTexNorm; 3]) -> (Vec3, Vec3) {
    let (v1, v2, v3) = (
        make_vec3(&face[0].position),
        make_vec3(&face[1].position),
        make_vec3(&face[2].position),
    );
    let (n1, n2, n3) = (
        make_vec3(&face[0].normal),
        make_vec3(&face[1].normal),
        make_vec3(&face[2].normal),
    );
    let (uv1, uv2, uv3) = (
        make_vec2(&face[0].tex_coord),
        make_vec2(&face[1].tex_coord),
        make_vec2(&face[2].tex_coord),
    );

    // compute average normal of vertices
    let normal = normalize(&(n1 + n2 + n3));

    // calculate edge length and UV differences
    let edge1 = v2 - v1;
    let edge2 = v3 - v1;
    let duv1 = uv2 - uv1;
    let duv2 = uv3 - uv1;

    // compute and bitangent
    let mut tangent = normalize(&vec3(
        duv2.y * edge1.x - duv1.y * edge2.x,
        duv2.y * edge1.y - duv1.y * edge2.y,
        duv2.y * edge1.z - duv1.y * edge2.z,
    ));

    tangent = normalize(&(tangent - dot(&tangent, &normal) * normal));
    let bitangent = tangent.cross(&normal);

    (tangent, bitangent)
}

pub fn load_obj(
    queue: Queue,
    render_pass: RenderPass,
    path: &Path,
) -> Vec<RenderableObject> {
    // loads every object in an OBJ file
    // each object has the following descriptors in custom_sets:
    //     - set 0, binding 0: basic material properties (ambient, diffuse, etc.)
    //     - set 0, binding 1: model matrix
    //     - set 1, bindings 0, 1 and 2: diffuse, specular and normal textures

    // create buffer for model matrix, used for all
    // for now just scales everything down to 1/10
    let model_data: [[f32; 4]; 4] =
        scale(&Mat4::identity().into(), &vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // create concrete pipeline, used to create descriptor sets for all_objects
    let vtype = VertexType {
        phantom: PhantomData::<PosTexNormTan>,
    };
    let pipeline_spec = PipelineSpec {
        vs_path: relative_path("shaders/obj-viewer/vert.glsl"),
        fs_path: relative_path("shaders/obj-viewer/frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        depth: true,
        vtype: Arc::new(vtype),
    };
    let pipeline = pipeline_spec.concrete(queue.device().clone(), render_pass);

    // load
    let obj = tobj::load_obj(path).unwrap();
    let raw_meshes: Vec<tobj::Mesh> = obj.0.iter().map(|model| model.mesh.clone()).collect();
    // (Mesh, material_idx)
    let meshes: Vec<(Mesh<PosTexNormTan>, usize)> = raw_meshes
        .iter()
        .map(|mesh| (convert_mesh(mesh), mesh.material_id.unwrap_or(0)))
        .collect();

    // create material buffers and load textures
    let raw_materials = obj.1;
    let materials: Vec<_> = raw_materials
        .iter()
        .map(|mat| {
            bufferize_data(
                queue.clone(),
                Material {
                    ambient: [mat.ambient[0], mat.ambient[1], mat.ambient[2], 0.0],
                    diffuse: [mat.diffuse[0], mat.diffuse[1], mat.diffuse[2], 0.0],
                    specular: [mat.specular[0], mat.specular[1], mat.specular[2], 0.0],
                    shininess: mat.shininess,
                },
            )
        })
        .collect();

    let sampler = default_sampler(queue.device().clone());

    let textures: Vec<_> = raw_materials
        .iter()
        .map(|mat| {
            let diff_path = if mat.diffuse_texture == "" {
                relative_path("textures/missing.png")
            } else {
                relative_path(&format!("meshes/sponza/{}", mat.diffuse_texture))
            };

            let spec_path = if mat.specular_texture == "" {
                relative_path("textures/missing-spec.png")
            } else {
                relative_path(&format!("meshes/sponza/{}", mat.specular_texture))
            };

            let normal_path = if mat.normal_texture == "" {
                relative_path("textures/missing.png")
            } else {
                relative_path(&format!("meshes/sponza/{}", mat.normal_texture))
            };

            let diff_tex = load_texture(queue.clone(), &diff_path, Format::R8G8B8A8Srgb);
            let spec_tex = load_texture(queue.clone(), &spec_path, Format::R8G8B8A8Unorm);
            let norm_tex = load_texture(queue.clone(), &normal_path, Format::R8G8B8A8Unorm);
            pds_for_images(sampler.clone(), pipeline.clone(), &[diff_tex, spec_tex, norm_tex], 1).unwrap()
        })
        .collect();

    // process
    meshes
        .iter()
        .map(|(mesh, material_idx)| {
            ObjectPrototype {
                vs_path: relative_path("shaders/obj-viewer/vert.glsl"),
                fs_path: relative_path("shaders/obj-viewer/frag.glsl"),
                fill_type: PrimitiveTopology::TriangleList,
                depth_buffer: true,
                mesh: mesh.clone(),
                custom_sets: vec![
                    pds_for_buffers(
                        pipeline.clone(),
                        &[materials[*material_idx].clone(), model_buffer.clone()],
                        0,
                    )
                    .unwrap(),
                    textures[*material_idx].clone(),
                ],
            }
            .into_renderable_object(queue.clone())
        })
        .collect()
}

fn convert_mesh(mesh: &tobj::Mesh) -> Mesh<PosTexNormTan> {
    let mut vertices = vec![];
    for i in 0..mesh.positions.len() / 3 {
        let position = [
            mesh.positions[i * 3],
            mesh.positions[i * 3 + 1],
            mesh.positions[i * 3 + 2],
        ];
        let normal = [
            mesh.normals[i * 3],
            mesh.normals[i * 3 + 1],
            mesh.normals[i * 3 + 2],
        ];
        let tex_coord = if mesh.texcoords.len() <= i * 2 + 1 {
            [0.0, 0.0]
        } else {
            [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1] * -1.0]
        };

        vertices.push(PosTexNorm {
            position,
            tex_coord,
            normal,
        });
    }

    let base_mesh = Mesh {
        vertices,
        indices: mesh.indices.clone(),
    };

    add_tangents(&base_mesh)
}

#[derive(Default, Debug, Clone, Copy)]
pub struct PosTexNorm {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
    pub normal: [f32; 3],
}
vulkano::impl_vertex!(PosTexNorm, position, tex_coord, normal);

#[derive(Default, Debug, Clone, Copy)]
pub struct PosTexNormTan {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
}
vulkano::impl_vertex!(PosTexNormTan, position, tex_coord, normal, tangent);

#[allow(dead_code)]
struct Material {
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
    shininess: f32,
}
