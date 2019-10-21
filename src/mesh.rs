use render_engine::mesh::Mesh;
use std::path::Path;

use nalgebra_glm::*;

pub fn load_obj(path: &Path) -> Mesh<PosTexNorm> {
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
