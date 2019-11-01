use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::mesh::ObjectPrototype;
use re::render_passes;
use re::system::{Pass, System};
use re::utils::bufferize_data;
use re::window::Window;
use re::mesh::PrimitiveTopology;

use nalgebra_glm::*;

use std::collections::HashMap;

use tests_render_engine::mesh::{convert_meshes, load_obj};
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
        // custom images, we use none
        HashMap::new(),
        "resolve_color",
    );

    window.set_render_pass(render_pass.clone());

    // create buffer and set for model matrix
    let model_data: [[f32; 4]; 4] = Mat4::identity().into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load, create pipeline spec and set for model matrix
    // only load 1st object
    let (mut models, _materials) = load_obj(&relative_path("meshes/dragon.obj")).expect("couldn't load OBJ");
    let mesh = convert_meshes(&[models.remove(0)]).remove(0);

    // TODO: move no-app shaders to base
    let mut object = ObjectPrototype {
        vs_path: relative_path("shaders/no-app/vert.glsl"),
        fs_path: relative_path("shaders/no-app/frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        mesh,
        custom_sets: vec![],    // will be filled in later
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    let pipeline = object.pipeline_spec.concrete(device.clone(), render_pass.clone());
    let model_set = pds_for_buffers(pipeline.clone(), &[model_buffer], 0).unwrap();
    object.custom_sets = vec![model_set];

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
