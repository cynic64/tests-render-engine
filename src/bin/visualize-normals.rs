use render_engine as re;

/*
Annoyances:
Why do I have to manage queue and device? :(
*/

use re::collection_cache::pds_for_buffers;
use re::mesh::{Mesh, ObjectSpec, Vertex3D};
use re::render_passes;
use re::system::{Pass, System};
use re::utils::bufferize_data;
use re::window::Window;
use re::PrimitiveTopology;

use nalgebra_glm::*;

use std::collections::HashMap;
use std::sync::Arc;

use tests_render_engine::*;

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
        &scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)),
        &vec3(0.0, -6.0, 0.0),
    )
    .into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load meshes and create objects
    let (raptor_mesh, raptor_verts) = load_obj(&relative_path("meshes/cube.obj"));
    let normals_mesh = normals_vis(&raptor_verts);
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

    // used in main loop
    let mut all_objects = HashMap::new();
    let raptor_pipe = raptor
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let normals_pipe = normals
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let raptor_set = pds_for_buffers(
            raptor_pipe.clone(),
            &[model_buffer.clone(), camera_buffer.clone()],
            0,
        )
        .unwrap(); // 0 is the descriptor set idx
        raptor.custom_set = Some(raptor_set);

        let normals_set = pds_for_buffers(
            normals_pipe.clone(),
            &[model_buffer.clone(), camera_buffer.clone()],
            0,
        )
        .unwrap(); // 0 is the descriptor set idx
        normals.custom_set = Some(normals_set);

        all_objects.insert("geometry", vec![raptor.clone(), normals.clone()]);

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

fn normals_vis(vertices: &[Vertex3D]) -> Mesh {
    let vertices: Vec<PosColorV> = vertices
        .iter()
        .flat_map(|v| {
            vec![
                PosColorV {
                    position: v.position,
                    color: [
                        v.normal[0] * 0.5 + 0.5,
                        v.normal[1] * 0.5 + 0.5,
                        v.normal[2] * 0.5 + 0.5,
                    ],
                },
                PosColorV {
                    position: [
                        v.position[0] + v.normal[0] * 0.2,
                        v.position[1] + v.normal[1] * 0.2,
                        v.position[2] + v.normal[2] * 0.2,
                    ],
                    color: [
                        v.normal[0] * 0.5 + 0.5,
                        v.normal[1] * 0.5 + 0.5,
                        v.normal[2] * 0.5 + 0.5,
                    ],
                },
            ]
        })
        .collect();
    let indices: Vec<u32> = (0..vertices.len()).map(|x| x as u32).collect();

    Mesh {
        vertices: Arc::new(vertices),
        indices,
    }
}

#[derive(Default, Debug, Clone)]
struct PosColorV {
    position: [f32; 3],
    color: [f32; 3],
}
vulkano::impl_vertex!(PosColorV, position, color);
