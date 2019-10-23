use render_engine as re;

use re::{Queue, Buffer};
use re::collection_cache::pds_for_buffers;
use re::render_passes;
use re::system::{Pass, System};
use re::utils::bufferize_data;
use re::window::Window;
use re::input::get_elapsed;

use std::collections::HashMap;

use tests_render_engine::{relative_path, FlyCamera};
use tests_render_engine::mesh::load_obj;

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

    // initialize camera
    let mut camera = FlyCamera::default();

    // light
    let light = MovingLight::new();

    // load objects
    let objects = load_obj(
        queue.clone(),
        render_pass.clone(),
        &relative_path("meshes/sponza/sponza.obj"),
    );
    println!("Objects Loaded: {}", objects.len());
    let mut all_objects = HashMap::new();

    // used in main loop
    let pipeline = objects[0]
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());

    while !window.update() {
        // update camera and light
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let light_buffer = light.get_buffer(queue.clone());
        let camera_light_set = pds_for_buffers(pipeline.clone(), &[camera_buffer, light_buffer], 2).unwrap(); // 0 is the descriptor set idx

        all_objects.insert("geometry", objects.clone().iter_mut().map(|obj| {
            // add camera set to each object before adding it to the scene
            obj.custom_sets.push(camera_light_set.clone());
            obj.clone()
        }).collect());

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

#[allow(dead_code)]
struct Light {
    direction: [f32; 4],
    power: f32,
}

struct MovingLight {
    start_time: std::time::Instant,
}

impl MovingLight {
    fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
        }
    }

    fn get_buffer(&self, queue: Queue) -> Buffer {
        let time = get_elapsed(self.start_time) / 4.0;
        let data = Light {
            direction: [time.sin(), 2.0, time.cos(), 0.0],
            power: 1.0,
        };

        bufferize_data(queue.clone(), data)
    }
}
