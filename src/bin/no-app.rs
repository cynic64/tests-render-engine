use render_engine as re;

/*
Annoyances:
Why do I have to manage queue and device? :(
*/

use re::collection_cache::pds_for_buffers;
use re::render_passes;
use re::system::{Pass, System};
use re::utils::bufferize_data;
use re::mesh::ObjectSpec;
use re::window::Window;

use nalgebra_glm::*;

use std::collections::HashMap;

use tests_render_engine::{OrbitCamera, relative_path};
use tests_render_engine::mesh::load_obj;

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let render_pass = render_passes::with_depth(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![Pass {
            name: "geometry",
            images_created_tags: vec!["color", "depth"],
            images_needed_tags: vec![],
            render_pass: render_pass.clone(),
        }],
        "color",
    );

    window.set_render_pass(render_pass.clone());

    // create buffer for model matrix
    let model_data: [[f32; 4]; 4] = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load mesh and create objec
    let mesh = load_obj(&relative_path("meshes/dragon.obj"));
    let mut object = ObjectSpec {
        vs_path: relative_path("shaders/no_app_vert.glsl"),
        fs_path: relative_path("shaders/no_app_frag.glsl"),
        mesh,
        depth_buffer: true,
        .. Default::default()
    }.build(queue.clone());

    // used in main loop
    let mut all_objects = HashMap::new();
    let pipeline = object.pipeline_spec.concrete(device.clone(), render_pass.clone());

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let set =
            pds_for_buffers(pipeline.clone(), &[model_buffer.clone(), camera_buffer], 0).unwrap(); // 0 is the descriptor set idx
        object.custom_set = Some(set);

        all_objects.insert("geometry", vec![object.clone()]);

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}
