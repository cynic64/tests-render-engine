use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::mesh::ObjectPrototype;
use re::mesh::{Mesh, PrimitiveTopology};
use re::system::{Pass, RenderableObject, System};
use re::utils::bufferize_data;
use re::window::Window;
use re::{render_passes, Format, Image, Queue, Pipeline};

use vulkano::command_buffer::DynamicState;
use vulkano::pipeline::viewport::Viewport;

use nalgebra_glm::*;

use std::collections::HashMap;
use std::path::Path;

use tests_render_engine::mesh::load_obj_single;
use tests_render_engine::{relative_path, OrbitCamera};

// patches are laid out in a 3x2
const SHADOW_MAP_DIMS: [u32; 2] = [3072, 2048];
const PATCH_DIMS: [f32; 2] = [1024.0, 1024.0];

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let patched_shadow_image: Image = vulkano::image::AttachmentImage::sampled(
        device.clone(),
        SHADOW_MAP_DIMS,
        Format::D32Sfloat,
    )
    .unwrap();
    let mut depth_view_custom_images = HashMap::new();
    depth_view_custom_images.insert("shadow_map", patched_shadow_image);

    let rpass1 = render_passes::only_depth(device.clone());
    let rpass2 = render_passes::basic(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![
            // renders to shadow cubemap
            Pass {
                name: "shadow",
                images_created_tags: vec!["shadow_map"],
                images_needed_tags: vec![],
                render_pass: rpass1.clone(),
                custom_images: HashMap::new(),
            },
            // displays shadow map for debugging
            Pass {
                name: "cubemap_view",
                images_created_tags: vec!["cubemap_view"],
                images_needed_tags: vec!["shadow_map"],
                render_pass: rpass2.clone(),
                custom_images: depth_view_custom_images,
            },
        ],
        "cubemap_view",
    );
    window.set_render_pass(rpass1.clone());

    // create buffer and set for model matrix
    let model_data: [[f32; 4]; 4] = Mat4::identity().into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load create pipeline spec and set for model matrix
    let mesh = load_obj_single(&relative_path("meshes/shadowtest.obj"));

    let custom_dynstate = DynamicState {
        line_width: None,
        viewports: Some(vec![Viewport {
            origin: [0.0, 0.0],
            dimensions: PATCH_DIMS,
            depth_range: 0.0..1.0,
        }]),
        scissors: None,
    };

    let mut base_dragon = ObjectPrototype {
        vs_path: relative_path("shaders/cubemap/shadow_cast_vert.glsl"),
        fs_path: relative_path("shaders/cubemap/shadow_cast_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        mesh,
        custom_sets: vec![], // will be filled in later
        custom_dynamic_state: Some(custom_dynstate),
    }
    .into_renderable_object(queue.clone());

    let pipe_dragon = base_dragon
        .pipeline_spec
        .concrete(device.clone(), rpass1.clone());
    let model_set = pds_for_buffers(pipe_dragon.clone(), &[model_buffer], 0).unwrap();
    base_dragon.custom_sets = vec![model_set];

    // create fullscreen quad to debug cubemap
    let quad = ObjectPrototype {
        vs_path: relative_path("shaders/cubemap/display_cubemap_vert.glsl"),
        fs_path: relative_path("shaders/cubemap/display_cubemap_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleStrip,
        read_depth: false,
        write_depth: false,
        mesh: Mesh {
            vertices: vec![
                V2D {
                    position: [-1.0, -1.0],
                },
                V2D {
                    position: [-1.0, 1.0],
                },
                V2D {
                    position: [1.0, -1.0],
                },
                V2D {
                    position: [1.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2, 3],
        },
        custom_sets: vec![],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    // create buffer for shadow projection matrix
    let (near, far) = (1.0, 250.0);
    // pi / 2 = 90 deg., 1.0 = aspect ratio
    let proj_data: [[f32; 4]; 4] = perspective(std::f32::consts::PI / 2.0, 1.0, near, far).into();
    let proj_buffer = bufferize_data(queue.clone(), proj_data);
    let proj_set = pds_for_buffers(pipe_dragon.clone(), &[proj_buffer], 1).unwrap();
    base_dragon.custom_sets.push(proj_set);

    // create 6 different dragon objects, each with a different view matrix and
    // dynamic state, to draw to the 6 different faces of the patched texture
    let dragons = convert_to_shadow_casters(queue.clone(), pipe_dragon, base_dragon);

    // used in main loop
    let mut all_objects = HashMap::new();
    all_objects.insert("shadow", dragons);
    all_objects.insert("cubemap_view", vec![quad]);

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

fn convert_to_shadow_casters(
    queue: Queue,
    pipeline: Pipeline,
    base_object: RenderableObject,
) -> Vec<RenderableObject> {
    // if you want to make point lamps cast shadows, you need shadow cubemaps
    // render-engine doesn't support geometry shaders, so the easiest way to do
    // this is to convert one object into 6 different ones, one for each face of
    // the cubemap, that each render to a different part of a 2D texture.
    // for now this function assumes a 3x2 patch layout
    let view_directions = [
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 0.0, 1.0),
        vec3(-1.0, 0.0, 0.0),
        vec3(0.0, -1.0, 0.0),
        vec3(0.0, 0.0, -1.0),
    ];

    let up_directions = [
        vec3(0.0, 1.0, 0.0),
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
    ];

    let patch_positions = [
        [0.0, 0.0],
        [1.0, 0.0],
        [2.0, 0.0],
        [0.0, 1.0],
        [1.0, 1.0],
        [2.0, 1.0],
    ];

    view_directions
        .iter()
        .zip(&up_directions)
        .zip(&patch_positions)
        .map(|((dir, up), patch_pos): ((&Vec3, &Vec3), &[f32; 2])| {
            let view_matrix: [[f32; 4]; 4] = look_at(
                &vec3(0.0, 0.0, 0.0), // light's position
                dir,
                up,
            )
            .into();
            let view_buffer = bufferize_data(queue.clone(), view_matrix);
            let set = pds_for_buffers(pipeline.clone(), &[view_buffer], 2).unwrap();

            // all sets for the dragon we're currently creating
            // we take the model and projection sets from the base dragon
            // (sets 0 and 1)
            let custom_sets = vec![
                base_object.custom_sets[0].clone(),
                base_object.custom_sets[1].clone(),
                set,
            ];

            // dynamic state for the current dragon, represents which part
            // of the patched texture we draw to
            let origin = [patch_pos[0] * PATCH_DIMS[0], patch_pos[1] * PATCH_DIMS[1]];
            let dynamic_state = dynamic_state_for_bounds(origin, PATCH_DIMS);

            RenderableObject {
                // model and proj are in set 0 and 1
                custom_sets,
                custom_dynamic_state: Some(dynamic_state),
                ..base_object.clone()
            }
        })
        .collect()
}

fn dynamic_state_for_bounds(origin: [f32; 2], dimensions: [f32; 2]) -> DynamicState {
    DynamicState {
        line_width: None,
        viewports: Some(vec![Viewport {
            origin,
            dimensions,
            depth_range: 0.0..1.0,
        }]),
        scissors: None,
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct V2D {
    position: [f32; 2],
}
vulkano::impl_vertex!(V2D, position);
