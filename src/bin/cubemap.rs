use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::mesh::ObjectPrototype;
use re::mesh::{Mesh, PrimitiveTopology};
use re::system::{Pass, System};
use re::utils::bufferize_data;
use re::window::Window;
use re::{render_passes, Format};

use nalgebra_glm::*;

use std::collections::HashMap;

use tests_render_engine::mesh::load_obj_single;
use tests_render_engine::{relative_path, OrbitCamera};

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let rpass1 = render_passes::with_depth(device.clone());
    let rpass2 = render_passes::basic(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![
            Pass {
                name: "geometry",
                images_created_tags: vec!["color", "depth"],
                images_needed_tags: vec![],
                render_pass: rpass1.clone(),
                custom_images: HashMap::new(),
            },
            Pass {
                name: "depth_view",
                images_created_tags: vec!["depth_view"],
                images_needed_tags: vec![],
                render_pass: rpass2.clone(),
                custom_images: HashMap::new(),
            },
        ],
        "depth_view",
    );

    /*
    let custom_depth = vulkano::image::AttachmentImage::sampled_multisampled(device.clone(), [1024, 1024], 1, Format::D32Sfloat)
        .unwrap();
    system.passes[0].custom_images.insert("depth", custom_depth);
    */

    window.set_render_pass(rpass1.clone());

    // create buffer and set for model matrix
    let model_data: [[f32; 4]; 4] = Mat4::identity().into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load create pipeline spec and set for model matrix
    let mesh = load_obj_single(&relative_path("meshes/dragon.obj"));

    let mut dragon = ObjectPrototype {
        vs_path: relative_path("shaders/cubemap/vert.glsl"),
        fs_path: relative_path("shaders/cubemap/frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        depth_buffer: true,
        mesh,
        custom_sets: vec![], // will be filled in later
    }
    .into_renderable_object(queue.clone());

    let pipeline = dragon
        .pipeline_spec
        .concrete(device.clone(), rpass1.clone());
    let model_set = pds_for_buffers(pipeline.clone(), &[model_buffer], 0).unwrap();
    dragon.custom_sets = vec![model_set];

    // create fullscreen quad
    let fullscreen = ObjectPrototype {
        vs_path: relative_path("shaders/cubemap/depth_view_vert.glsl"),
        fs_path: relative_path("shaders/cubemap/depth_view_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleStrip,
        depth_buffer: false,
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

    // used in main loop
    let mut all_objects = HashMap::new();
    all_objects.insert("depth_view", vec![fullscreen]);

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());

        let camera_buffer = camera.get_buffer(queue.clone());
        let camera_set = pds_for_buffers(pipeline.clone(), &[camera_buffer], 1).unwrap();

        // in the beginning, custom_sets only includes the model set. handle
        // both cases.
        if dragon.custom_sets.len() == 1 {
            dragon.custom_sets.push(camera_set);
        } else if dragon.custom_sets.len() == 2 {
            dragon.custom_sets[1] = camera_set;
        } else {
            panic!("weird custom set length");
        }

        // replace old "geometry" object list
        all_objects.insert("geometry", vec![dragon.clone()]);

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
