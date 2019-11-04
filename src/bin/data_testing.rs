use render_engine as re;

use re::mesh::ObjectPrototype;
use re::render_passes;
use re::system::{Pass, System};
use re::window::Window;
use re::mesh::{PrimitiveTopology, Mesh};

use nalgebra_glm::*;

use std::collections::HashMap;

use tests_render_engine::mesh::VPosColor2D;
use tests_render_engine::{relative_path, Matrix4};

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let render_pass = render_passes::basic(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![Pass {
            name: "geometry",
            images_created_tags: vec!["color"],
            images_needed_tags: vec![],
            render_pass: render_pass.clone(),
        }],
        // custom images, we use none
        HashMap::new(),
        "color",
    );

    window.set_render_pass(render_pass.clone());

    // create data for model matrix
    let data: Matrix4 = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)).into();

    // create object
    let object = ObjectPrototype {
        vs_path: relative_path("shaders/data-testing/vert.glsl"),
        fs_path: relative_path("shaders/data-testing/frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: false,
        write_depth: false,
        mesh: Mesh {
            vertices: vec![
                VPosColor2D {
                    position: [0.0, -1.0],
                    color: [1.0, 0.0, 0.0],
                },
                VPosColor2D {
                    position: [-1.0, 1.0],
                    color: [0.0, 1.0, 0.0],
                },
                VPosColor2D {
                    position: [1.0, 1.0],
                    color: [0.0, 0.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
        },
        collection: (
            (data,),
        ),
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    // used in main loop
    let mut all_objects = HashMap::new();
    all_objects.insert("geometry", vec![object.clone()]);

    while !window.update() {
        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}
