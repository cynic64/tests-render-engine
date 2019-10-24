use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::render_passes;
use re::system::{Pass, System};
use re::window::Window;

use std::collections::HashMap;

use tests_render_engine::mesh::load_obj_no_textures;
use tests_render_engine::{relative_path, FlyCamera};

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let rpass1 = render_passes::only_depth(device.clone());
    let rpass2 = render_passes::read_depth(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![
            Pass {
                name: "prepass",
                images_created_tags: vec!["depth"],
                images_needed_tags: vec![],
                render_pass: rpass1.clone(),
                custom_images: HashMap::new(),
            },
            /*
            Pass {
                name: "depth_view",
                images_created_tags: vec!["depth_view"],
                images_needed_tags: vec!["depth"],
                render_pass: rpass2.clone(),
                custom_images: HashMap::new(),
            },
            */
            Pass {
                name: "geometry",
                // should re-use old depth buffer
                images_created_tags: vec!["color", "depth"],
                images_needed_tags: vec![],
                render_pass: rpass2.clone(),
                custom_images: HashMap::new(),
            },
        ],
        "color",
    );

    window.set_render_pass(rpass1.clone());

    // initialize camera
    let mut camera = FlyCamera::default();

    // load objects with shaders for depth prepass
    let objects_depth = load_obj_no_textures(
        queue.clone(),
        rpass1.clone(),
        &relative_path("shaders/depth-prepass/vert.glsl"),
        &relative_path("shaders/depth-prepass/frag.glsl"),
        &relative_path("meshes/sponza/sponza.obj"),
    );
    let objects_geo: Vec<_> = objects_depth.iter().map(|obj| {
        let mut new_obj = obj.clone();
        new_obj.pipeline_spec.vs_path = relative_path("shaders/depth-prepass/object_vert.glsl");
        new_obj.pipeline_spec.fs_path = relative_path("shaders/depth-prepass/object_frag.glsl");
        new_obj.pipeline_spec.write_depth = false;
        new_obj
    })
    .collect();
    // needed for creating camera set
    let pipeline_depth = objects_depth[0].pipeline_spec.concrete(device.clone(), rpass1.clone());
    let pipeline_color = objects_geo[0].pipeline_spec.concrete(device.clone(), rpass2.clone());

    // create fullscreen quad
    /*
    let fullscreen = ObjectPrototype {
        vs_path: relative_path("shaders/depth-prepass/depth_view_vert.glsl"),
        fs_path: relative_path("shaders/depth-prepass/depth_view_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleStrip,
        read_depth: false,
        write_depth: false,
        mesh: Mesh {
            vertices: vec![
                V2D {
                    tex_coords: [-1.0, -1.0],
                },
                V2D {
                    tex_coords: [-1.0, 1.0],
                },
                V2D {
                    tex_coords: [1.0, -1.0],
                },
                V2D {
                    tex_coords: [1.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2, 3],
        },
        custom_sets: vec![],
    }
    .into_renderable_object(queue.clone());
    */

    // used in main loop
    let mut all_objects = HashMap::new();
    // all_objects.insert("depth_view", vec![fullscreen]);

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());

        let camera_buffer = camera.get_buffer(queue.clone());
        let camera_set_depth = pds_for_buffers(pipeline_depth.clone(), &[camera_buffer.clone()], 1).unwrap();
        let camera_set_geo = pds_for_buffers(pipeline_color.clone(), &[camera_buffer.clone()], 1).unwrap();

        // add camera sets to objects and objects into all_objects
        all_objects.insert("prepass", objects_depth
                           .iter()
                           .map(|obj| {
                               let mut new_obj = obj.clone();
                               new_obj.custom_sets.push(camera_set_depth.clone());
                               new_obj
                           })
                           .collect());
        all_objects.insert("geometry", objects_geo
                           .iter()
                           .map(|obj| {
                               let mut new_obj = obj.clone();
                               new_obj.custom_sets.push(camera_set_geo.clone());
                               new_obj
                           })
                           .collect());

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

#[derive(Default, Debug, Clone, Copy)]
struct V2D {
    tex_coords: [f32; 2],
}
vulkano::impl_vertex!(V2D, tex_coords);
