use render_engine as re;

/*
Annoyances:
Why do I have to manage queue and device? :(
*/

use re::collection_cache::pds_for_buffers;
use re::mesh::{Mesh, ObjectSpec, Vertex3D};
use re::render_passes;
use re::system::{Pass, System};
use re::utils::{bufferize_data, load_texture};
use re::window::Window;
use re::PrimitiveTopology;
use re::input::VirtualKeyCode;

use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;

use nalgebra_glm::*;

use std::collections::HashMap;
use std::sync::Arc;

use tests_render_engine::{load_obj, relative_path, OrbitCamera, default_sampler};

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let render_pass = render_passes::multisampled_with_depth(device.clone(), 4);
    let mut system = System::new(
        queue.clone(),
        vec![Pass {
            name: "geometry",
            images_created_tags: vec![
                "resolve_color",
                "multisampeld_color",
                "multisampled_depth",
                "resolve_depth",
            ],
            images_needed_tags: vec![],
            render_pass: render_pass.clone(),
        }],
        "resolve_color",
    );

    window.set_render_pass(render_pass.clone());

    // create buffer for model matrix
    let model_data: [[f32; 4]; 4] = translate(
        &scale(&Mat4::identity(), &vec3(1.0, 1.0, 1.0)),
        &vec3(0.0, -6.0, 0.0),
    )
    .into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load meshes and create objects
    let (verts, indices) = load_obj(&relative_path("meshes/raptor.obj"));
    let (raptor_verts, raptor_indices) = ptnt_from(&verts, &indices);

    let normals_mesh = normals_vis(&raptor_verts, &raptor_indices);
    let raptor_mesh = Mesh {
        vertices: Arc::new(raptor_verts),
        indices: raptor_indices,
    };

    let mut raptor = ObjectSpec {
        vs_path: relative_path("shaders/visualize-normals/object_vert.glsl"),
        fs_path: relative_path("shaders/visualize-normals/object_frag.glsl"),
        mesh: raptor_mesh,
        depth_buffer: true,
        ..Default::default()
    }
    .build(queue.clone());

    let mut normals = ObjectSpec {
        vs_path: relative_path("shaders/visualize-normals/debug_vert.glsl"),
        fs_path: relative_path("shaders/visualize-normals/debug_frag.glsl"),
        mesh: normals_mesh,
        depth_buffer: true,
        fill_type: PrimitiveTopology::LineList,
        ..Default::default()
    }
    .build(queue.clone());

    // textures
    let normal_texture = load_texture(queue.clone(), &relative_path("textures/raptor-normal.png"));

    // used in main loop
    let mut all_objects = HashMap::new();
    let raptor_pipe = raptor
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let normals_pipe = normals
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let sampler = default_sampler(device.clone());

    let mut debug: bool = false;

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        raptor.custom_set = Some(Arc::new(
            PersistentDescriptorSet::start(raptor_pipe.clone(), 0)
                .add_buffer(model_buffer.clone())
                .unwrap()
                .add_buffer(camera_buffer.clone())
                .unwrap()
                .add_sampled_image(normal_texture.clone(), sampler.clone())
                .unwrap()
                .build()
                .unwrap(),
        ));

        let normals_set = pds_for_buffers(
            normals_pipe.clone(),
            &[model_buffer.clone(), camera_buffer.clone()],
            0,
        )
        .unwrap(); // 0 is the descriptor set idx
        normals.custom_set = Some(normals_set);

        all_objects.insert("geometry", vec![raptor.clone(), normals.clone()]);

        if window.get_frame_info().keydowns.contains(&VirtualKeyCode::C) {
            debug = !debug;
            if debug {
                raptor.pipeline_spec.fs_path = relative_path("shaders/visualize-normals/object_frag_normal.glsl")
            } else {
                raptor.pipeline_spec.fs_path = relative_path("shaders/visualize-normals/object_frag.glsl");
            }
        }

        // draw
        // TODO: maybe make system take a mut pointer to window instead?
        let swapchain_image = window.next_image();
        let swapchain_fut = window.get_future();

        // draw_frame returns a future representing the completion of rendering
        let frame_fut = system.draw_frame(
            swapchain_image.dimensions(),
            all_objects.clone(),
            swapchain_image,
            swapchain_fut,
        );

        window.present_future(frame_fut);
    }

    println!("FPS: {}", window.get_fps());
    system.print_stats();
}

fn ptnt_from(vertices: &[Vertex3D], indices: &[u32]) -> (Vec<PosTexNormTan>, Vec<u32>) {
    let mut tangents: Vec<Vec3> = vec![vec3(0.0, 0.0, 0.0); vertices.len()];
    let mut bitangents: Vec<Vec3> = vec![vec3(0.0, 0.0, 0.0); vertices.len()];

    for i in 0..indices.len() / 3 {
        let face = [
            vertices[indices[i * 3] as usize],
            vertices[indices[i * 3 + 1] as usize],
            vertices[indices[i * 3 + 2] as usize],
        ];
        let (tangent, bitangent) = tangent_bitangent_for_face(&face);
        tangents[indices[i * 3] as usize] += tangent;
        tangents[indices[i * 3 + 1] as usize] += tangent;
        tangents[indices[i * 3 + 2] as usize] += tangent;

        bitangents[indices[i * 3] as usize] += bitangent;
        bitangents[indices[i * 3 + 1] as usize] += bitangent;
        bitangents[indices[i * 3 + 2] as usize] += bitangent;
    }

    let new_vertices: Vec<PosTexNormTan> = vertices
        .iter()
        .enumerate()
        .map(|(idx, v)| {
            let n = vec3(v.normal[0], v.normal[1], v.normal[2]);
            let t0 = normalize(&tangents[idx]);
            let t1 = normalize(&bitangents[idx]);

            PosTexNormTan {
                position: v.position,
                tex_coord: v.tex_coord,
                normal: v.normal,
                tangent: t0.into(),
                bitangent: t1.into(),
            }
        })
        .collect();

    (new_vertices, indices.to_vec())
}

fn normals_vis(vertices: &[PosTexNormTan], indices: &[u32]) -> Mesh {
    let faces = faces_from(vertices, indices);
    let wireframe = faces.iter().flat_map(|f| {
        vec![
            PosColorV {
                position: f[0].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColorV {
                position: f[1].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColorV {
                position: f[1].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColorV {
                position: f[2].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColorV {
                position: f[2].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColorV {
                position: f[0].position,
                color: [0.5, 0.5, 0.5],
            },
        ]
    });

    let vertices: Vec<PosColorV> = vertices
        .iter()
        .flat_map(|v| {
            vec![
                PosColorV {
                    position: v.position,
                    color: [1.0, 0.0, 0.0],
                },
                PosColorV {
                    position: [
                        v.position[0] + v.normal[0] * 0.2,
                        v.position[1] + v.normal[1] * 0.2,
                        v.position[2] + v.normal[2] * 0.2,
                    ],
                    color: [1.0, 0.0, 0.0],
                },
                PosColorV {
                    position: v.position,
                    color: [0.0, 1.0, 0.0],
                },
                PosColorV {
                    position: [
                        v.position[0] + v.tangent[0] * 0.2,
                        v.position[1] + v.tangent[1] * 0.2,
                        v.position[2] + v.tangent[2] * 0.2,
                    ],
                    color: [0.0, 1.0, 0.0],
                },
                PosColorV {
                    position: v.position,
                    color: [0.0, 0.0, 1.0],
                },
                PosColorV {
                    position: [
                        v.position[0] + v.bitangent[0] * 0.2,
                        v.position[1] + v.bitangent[1] * 0.2,
                        v.position[2] + v.bitangent[2] * 0.2,
                    ],
                    color: [0.0, 0.0, 1.0],
                },
            ]
        })
        .chain(wireframe)
        .collect();

    let indices: Vec<u32> = (0..vertices.len()).map(|x| x as u32).collect();

    Mesh {
        vertices: Arc::new(vertices),
        indices,
    }
}

fn tangent_bitangent_for_face(face: &[Vertex3D; 3]) -> (Vec3, Vec3) {
    // compute average normal of vertices
    let normal = normalize(&vec3(
        face[0].normal[0] + face[1].normal[0] + face[2].normal[0],
        face[0].normal[1] + face[1].normal[1] + face[2].normal[1],
        face[0].normal[2] + face[1].normal[2] + face[2].normal[2],
    ));

    // calculate edge length and UV differences
    // edge1 = vertex2 - vertex1
    let edge1 = vec3(
        face[1].position[0] - face[0].position[0],
        face[1].position[1] - face[0].position[1],
        face[1].position[2] - face[0].position[2],
    );
    // edge2 = vertex3 - vertex1
    let edge2 = vec3(
        face[2].position[0] - face[0].position[0],
        face[2].position[1] - face[0].position[1],
        face[2].position[2] - face[0].position[2],
    );
    // duv1 = uv2 - uv1
    let duv1 = vec2(
        face[1].tex_coord[0] - face[0].tex_coord[0],
        face[1].tex_coord[1] - face[0].tex_coord[1],
    );
    // duv2 = uv3 - uv1
    let duv2 = vec2(
        face[2].tex_coord[0] - face[0].tex_coord[0],
        face[2].tex_coord[1] - face[0].tex_coord[1],
    );

    // compute and bitangent
    let mut tangent = normalize(&vec3(
        duv2.y * edge1.x - duv1.y * edge2.x,
        duv2.y * edge1.y - duv1.y * edge2.y,
        duv2.y * edge1.z - duv1.y * edge2.z,
    ));

    tangent = normalize(&(tangent - dot(&tangent, &normal) * normal));
    let bitangent = tangent.cross(&normal);

    /*
    let bitangent = normalize(&vec3(
        -duv2.x * edge1.x - duv1.x * edge2.x,
        -duv2.x * edge1.y - duv1.x * edge2.y,
        -duv2.x * edge1.z - duv1.x * edge2.z,
    ));
    */

    (tangent, bitangent)
}

fn faces_from(vertices: &[PosTexNormTan], indices: &[u32]) -> Vec<Face> {
    let mut faces = vec![];
    for i in 0..indices.len() / 3 {
        faces.push([
            vertices[indices[i * 3] as usize],
            vertices[indices[i * 3 + 1] as usize],
            vertices[indices[i * 3 + 2] as usize],
        ])
    }

    faces
}

type Face = [PosTexNormTan; 3];

#[derive(Default, Debug, Clone, Copy)]
struct PosColorV {
    position: [f32; 3],
    color: [f32; 3],
}
vulkano::impl_vertex!(PosColorV, position, color);

#[derive(Default, Debug, Clone, Copy)]
struct PosTexNormTan {
    position: [f32; 3],
    tex_coord: [f32; 2],
    normal: [f32; 3],
    tangent: [f32; 3],
    bitangent: [f32; 3],
}
vulkano::impl_vertex!(PosTexNormTan, position, tex_coord, normal, tangent, bitangent);
