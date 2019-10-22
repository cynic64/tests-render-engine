use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::mesh::{RenderableObjectSpec, VertexType};
use re::pipeline_cache::PipelineSpec;
use re::render_passes;
use re::system::{Pass, System};
use re::utils::bufferize_data;
use re::window::Window;

use nalgebra_glm::*;

use std::collections::HashMap;

use tests_render_engine::mesh::{load_obj, PosTexNorm};
use tests_render_engine::{relative_path, OrbitCamera};

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
                "multisampled_color",
                "multisampled_depth",
                "resolve_depth",
            ],
            images_needed_tags: vec![],
            render_pass: render_pass.clone(),
        }],
        "resolve_color",
    );

    window.set_render_pass(render_pass.clone());

    // create buffer and set for model matrix
    let model_data: [[f32; 4]; 4] = Mat4::identity().into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load create pipeline spec and set for model matrix
    let mesh = load_obj(&relative_path("meshes/dragon.obj"));
    let pipeline_spec = PipelineSpec {
        vs_path: relative_path("shaders/no-app/vert.glsl"),
        fs_path: relative_path("shaders/no-app/frag.glsl"),
        depth: true,
        vtype: VertexType::<PosTexNorm>::new(),
        // TODO: make default take vtype as type param, because it is always
        // needed
        ..Default::default()
    };

    // needed for creating descriptor sets
    // maybe add set creation functions to pipeline_spec or system?
    let pipeline = pipeline_spec.concrete(device.clone(), render_pass.clone());
    let model_set = pds_for_buffers(pipeline.clone(), &[model_buffer], 0).unwrap();

    let mut object = RenderableObjectSpec {
        pipeline_spec,
        mesh,
        custom_sets: vec![model_set],
        ..Default::default()
    }
    .build(queue.clone());

    // used in main loop
    let mut all_objects = HashMap::new();

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());

        let camera_buffer = camera.get_buffer(queue.clone());
        let camera_set = pds_for_buffers(pipeline.clone(), &[camera_buffer], 1).unwrap();

        // in the beginning, custom_sets only includes the model set. handle
        // both cases.
        if object.custom_sets.len() == 1 {
            object.custom_sets.push(camera_set);
        } else if object.custom_sets.len() == 2 {
            object.custom_sets[1] = camera_set;
        } else {
            panic!("weird custom set length");
        }

        // replace old "geometry" object list
        all_objects.insert("geometry", vec![object.clone()]);

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}
