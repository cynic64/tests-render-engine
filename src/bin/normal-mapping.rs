use render_engine as re;

/*
Annoyances:
Why do I have to manage queue and device? :(
*/

use re::collection_cache::pds_for_buffers;
use re::input::{get_elapsed, VirtualKeyCode};
use re::mesh::{Mesh, ObjectSpec};
use re::render_passes;
use re::system::{Pass, System};
use re::utils::{bufferize_data, load_texture};
use re::window::Window;
use re::PrimitiveTopology;

use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;

use nalgebra_glm::*;

use std::collections::HashMap;
use std::sync::Arc;

use tests_render_engine::mesh::{add_tangents, load_obj, PosTexNormTan};
use tests_render_engine::{default_sampler, relative_path, OrbitCamera};

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
    let basic_mesh = load_obj(&relative_path("meshes/raptor.obj"));
    let raptor_mesh = add_tangents(&basic_mesh);
    let normals_mesh = normals_vis(&raptor_mesh);

    let mut raptor = ObjectSpec {
        vs_path: relative_path("shaders/normal-mapping/object_vert.glsl"),
        fs_path: relative_path("shaders/normal-mapping/object_frag.glsl"),
        mesh: raptor_mesh,
        depth_buffer: true,
        ..Default::default()
    }
    .build(queue.clone());

    let mut normals = ObjectSpec {
        vs_path: relative_path("shaders/normal-mapping/debug_vert.glsl"),
        fs_path: relative_path("shaders/normal-mapping/debug_frag.glsl"),
        mesh: normals_mesh,
        depth_buffer: true,
        fill_type: PrimitiveTopology::LineList,
        ..Default::default()
    }
    .build(queue.clone());

    // textures
    let normal_texture = load_texture(queue.clone(), &relative_path("textures/raptor-normal.png"));

    // light
    let mut light = Light {
        position: [10.0, 0.0, 0.0],
    };

    // used in main loop
    let mut all_objects = HashMap::new();
    let raptor_pipe = raptor
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let normals_pipe = normals
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let sampler = default_sampler(device.clone());
    let start_time = std::time::Instant::now();

    let mut debug: bool = false;

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        // update light
        let time = get_elapsed(start_time);
        let light_x = (time / 4.0).sin() * 20.0;
        let light_z = (time / 4.0).cos() * 20.0;
        light.position = [light_x, 0.0, light_z];
        let light_buffer = bufferize_data(queue.clone(), light.clone());

        // create set
        raptor.custom_set = Some(Arc::new(
            PersistentDescriptorSet::start(raptor_pipe.clone(), 0)
                .add_buffer(model_buffer.clone())
                .unwrap()
                .add_buffer(camera_buffer.clone())
                .unwrap()
                .add_buffer(light_buffer.clone())
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

        if window
            .get_frame_info()
            .keydowns
            .contains(&VirtualKeyCode::C)
        {
            debug = !debug;
            if debug {
                raptor.pipeline_spec.fs_path =
                    relative_path("shaders/normal-mapping/object_frag_debug.glsl");
            } else {
                raptor.pipeline_spec.fs_path =
                    relative_path("shaders/normal-mapping/object_frag.glsl");
            }
        }

        let objects = if debug {
            vec![raptor.clone(), normals.clone()]
        } else {
            vec![raptor.clone()]
        };
        all_objects.insert("geometry", objects);

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
    system.print_stats();
}

fn normals_vis(mesh: &Mesh<PosTexNormTan>) -> Mesh<PosColor> {
    let (vertices, indices) = (&mesh.vertices, &mesh.indices);

    let faces = faces_from(&vertices, &indices);
    let wireframe = faces.iter().flat_map(|f| {
        vec![
            PosColor {
                position: f[0].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColor {
                position: f[1].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColor {
                position: f[1].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColor {
                position: f[2].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColor {
                position: f[2].position,
                color: [0.5, 0.5, 0.5],
            },
            PosColor {
                position: f[0].position,
                color: [0.5, 0.5, 0.5],
            },
        ]
    });

    let vertices: Vec<PosColor> = vertices
        .iter()
        .flat_map(|v| {
            let normal = make_vec3(&v.normal);
            let tangent = make_vec3(&v.tangent);
            let bitangent = tangent.cross(&normal);
            let position = make_vec3(&v.position);

            vec![
                PosColor {
                    position: v.position,
                    color: [1.0, 0.0, 0.0],
                },
                PosColor {
                    position: (position + normal * 0.2).into(),
                    color: [1.0, 0.0, 0.0],
                },
                PosColor {
                    position: v.position,
                    color: [0.0, 1.0, 0.0],
                },
                PosColor {
                    position: (position + tangent * 0.2).into(),
                    color: [0.0, 1.0, 0.0],
                },
                PosColor {
                    position: v.position,
                    color: [0.0, 0.0, 1.0],
                },
                PosColor {
                    position: (position + bitangent * 0.2).into(),
                    color: [0.0, 0.0, 1.0],
                },
            ]
        })
        .chain(wireframe)
        .collect();

    let indices: Vec<u32> = (0..vertices.len()).map(|x| x as u32).collect();

    Mesh { vertices, indices }
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

#[derive(Clone)]
struct Light {
    position: [f32; 3],
}

#[derive(Default, Debug, Clone, Copy)]
struct PosColor {
    position: [f32; 3],
    color: [f32; 3],
}
vulkano::impl_vertex!(PosColor, position, color);
