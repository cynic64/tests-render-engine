use render_engine::collection_cache::{pds_for_buffers, pds_for_images};
use render_engine::mesh::{Mesh, ObjectPrototype, PrimitiveTopology, VertexType};
use render_engine::pipeline_cache::PipelineSpec;
use render_engine::system::RenderableObject;
use render_engine::utils::{bufferize_data, load_texture};
use render_engine::{Buffer, Format, Queue, RenderPass, Set};

use nalgebra_glm::*;

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{default_sampler, relative_path};

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

pub fn load_obj(queue: Queue, render_pass: RenderPass, path: &Path, vs_path: PathBuf, fs_path: PathBuf, textures_set_idx: usize) -> Vec<RenderableObject> {
    // loads every object in an OBJ file
    // each object has the following descriptors in custom_sets:
    //     - set 0, binding 0: basic material properties (ambient, diffuse, etc.)
    //     - set 0, binding 1: model matrix
    //     - set 1, bindings 0, 1 and 2: diffuse, specular and normal textures

    // create buffer for model matrix, used for all
    let model_data: [[f32; 4]; 4] = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // create concrete pipeline, used to create descriptor sets for all_objects
    let vtype = VertexType {
        phantom: PhantomData::<PosTexNormTan>,
    };
    let pipeline_spec = PipelineSpec {
        vs_path: vs_path.clone(),
        fs_path: fs_path.clone(),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        vtype: Arc::new(vtype),
    };
    let pipeline = pipeline_spec.concrete(queue.device().clone(), render_pass);

    // load
    let obj = tobj::load_obj(path).unwrap();
    let raw_meshes: Vec<tobj::Mesh> = obj.0.iter().map(|model| model.mesh.clone()).collect();
    // (Mesh, material_idx)
    let meshes: Vec<(Mesh<PosTexNormTan>, usize)> = raw_meshes
        .iter()
        .map(|mesh| (add_tangents(&convert_mesh(mesh)), mesh.material_id.unwrap_or(0)))
        .collect();

    // create material buffers and load textures
    let raw_materials = obj.1;

    let sampler = default_sampler(queue.device().clone());
    let mat_buffers_and_texture_sets: Vec<(Buffer, Set)> = raw_materials
        .iter()
        .map(|mat| {
            // first try to load textures
            let parent_path = path.parent().expect(&format!("couldn't get parent of path: {:?}", path));
            let maybe_diff_path = parent_path.join(Path::new(&mat.diffuse_texture));
            let (diff_path, diff_found) = if mat.diffuse_texture == "" || !maybe_diff_path.exists() {
                (relative_path("textures/missing.png"), 0.0)
            } else {
                (maybe_diff_path, 1.0)
            };

            let maybe_spec_path = parent_path.join(Path::new(&mat.specular_texture));
            let spec_path = if mat.specular_texture == "" || !maybe_spec_path.exists() {
                relative_path("textures/missing-spec.png")
            } else {
                maybe_spec_path
            };

            let maybe_normal_path = parent_path.join(Path::new(&mat.normal_texture));
            let normal_path = if mat.normal_texture == "" || !maybe_normal_path.exists() {
                relative_path("textures/missing-normal.png")
            } else {
                maybe_normal_path
            };

            let diff_tex = load_texture(queue.clone(), &diff_path, Format::R8G8B8A8Srgb);
            let spec_tex = load_texture(queue.clone(), &spec_path, Format::R8G8B8A8Unorm);
            let norm_tex = load_texture(queue.clone(), &normal_path, Format::R8G8B8A8Unorm);
            let textures_set = pds_for_images(
                sampler.clone(),
                pipeline.clone(),
                &[diff_tex, spec_tex, norm_tex],
                textures_set_idx,
            )
                .unwrap();

            // sometimes shininess isn't specified and we need to provide a
            // sensible default
            let shininess = if mat.shininess > 1.0 {
                mat.shininess
            } else {
                32.0
            };

            let material_data = Material {
                ambient: [mat.ambient[0], mat.ambient[1], mat.ambient[2], 0.0],
                diffuse: [mat.diffuse[0], mat.diffuse[1], mat.diffuse[2], 0.0],
                specular: [mat.specular[0], mat.specular[1], mat.specular[2], 0.0],
                shininess: [shininess, 0.0, 0.0, 0.0],
                use_texture: [diff_found, 0.0, 0.0, 0.0],
            };
            let material_buffer: Buffer = bufferize_data(queue.clone(), material_data);

            (material_buffer, textures_set)
        })
        .collect();

    // process
    let (mut total_verts, mut total_indices) = (0, 0);
    let objects = meshes
        .iter()
        .map(|(mesh, material_idx)| {
            total_verts += mesh.vertices.len();
            total_indices += mesh.indices.len();

            let material_buffer = mat_buffers_and_texture_sets[*material_idx].0.clone();
            let texture_set = mat_buffers_and_texture_sets[*material_idx].1.clone();

            ObjectPrototype {
                vs_path: vs_path.clone(),
                fs_path: fs_path.clone(),
                fill_type: PrimitiveTopology::TriangleList,
                read_depth: true,
                write_depth: true,
                mesh: mesh.clone(),
                custom_sets: vec![
                    pds_for_buffers(
                        pipeline.clone(),
                        &[material_buffer, model_buffer.clone()],
                        textures_set_idx - 1,
                    )
                    .unwrap(),
                    texture_set,
                ],
                custom_dynamic_state: None,
            }
            .into_renderable_object(queue.clone())
        })
        .collect();

    println!("Total vertices: {}", total_verts);
    println!("Total indices: {}", total_indices);

    objects
}

pub fn load_obj_no_textures(queue: Queue, render_pass: RenderPass, vs_path: &Path, fs_path: &Path, obj_path: &Path) -> Vec<RenderableObject> {
    // loads all objects in an obj file without loading any textures.
    // only set included: model buffer at set 0, binding 0

    // get concrete pipeline, needed to create set later
    let pipeline_spec = PipelineSpec {
        vs_path: vs_path.to_path_buf(),
        fs_path: fs_path.to_path_buf(),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        vtype: VertexType::<PosTexNorm>::new(),
    };
    let pipeline = pipeline_spec.concrete(queue.device().clone(), render_pass);

    // create model set
    let model_data = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1));
    let model_buffer = bufferize_data(queue.clone(), model_data);
    let model_set = pds_for_buffers(pipeline, &[model_buffer], 0).unwrap();

    // load meshes
    let obj = tobj::load_obj(obj_path).unwrap();
    let raw_meshes: Vec<tobj::Mesh> = obj.0.iter().map(|model| model.mesh.clone()).collect();
    let meshes: Vec<Mesh<PosTexNorm>> = raw_meshes
        .iter()
        .map(|mesh| convert_mesh(mesh))
        .collect();

    // create renderable objects
    meshes
        .iter()
        .map(|mesh| ObjectPrototype {
            vs_path: vs_path.to_path_buf(),
            fs_path: fs_path.to_path_buf(),
            fill_type: PrimitiveTopology::TriangleList,
            read_depth: true,
            write_depth: true,
            mesh: mesh.clone(),
            custom_sets: vec![model_set.clone()],
            custom_dynamic_state: None,
        }.into_renderable_object(queue.clone()))
        .collect()
}

pub fn convert_mesh(mesh: &tobj::Mesh) -> Mesh<PosTexNorm> {
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

    Mesh {
        vertices,
        indices: mesh.indices.clone(),
    }
}

#[allow(dead_code)]
struct Material {
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
    shininess: [f32; 4],
    use_texture: [f32; 4],
}

pub fn merge(meshes: &[Mesh<PosTexNorm>]) -> Mesh<Pos> {
    // merges a list of meshes into a single mesh with only position data

    // you could probably write this as an iterator, i'm just too lazy
    let mut vertices = vec![];
    let mut indices = vec![];
    // we need to offset some indices because the vertices are being merged into
    // one giant list
    let mut index_offset = 0;
    for mesh in meshes.iter() {
        for vertex in mesh.vertices.iter() {
            // only copy position data
            vertices.push(Pos {
                position: vertex.position
            });
        }

        for index in mesh.indices.iter() {
            indices.push(index + index_offset);
        }

        index_offset += mesh.vertices.len() as u32;
    }

    Mesh {
        vertices,
        indices,
    }
}

pub fn fullscreen_quad(queue: Queue, vs_path: PathBuf, fs_path: PathBuf) -> RenderableObject {
    ObjectPrototype {
        vs_path,
        fs_path,
        fill_type: PrimitiveTopology::TriangleStrip,
        read_depth: false,
        write_depth: false,
        mesh: Mesh {
            vertices: vec![
                Vertex2D {
                    position: [-1.0, -1.0],
                },
                Vertex2D {
                    position: [-1.0, 1.0],
                },
                Vertex2D {
                    position: [1.0, -1.0],
                },
                Vertex2D {
                    position: [1.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2, 3],
        },
        custom_sets: vec![],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue)
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Vertex2D {
    pub position: [f32; 2],
}
vulkano::impl_vertex!(Vertex2D, position);

#[derive(Default, Debug, Clone, Copy)]
pub struct Pos {
    pub position: [f32; 3],
}

vulkano::impl_vertex!(Pos, position);

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
