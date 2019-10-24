use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::mesh::ObjectPrototype;
use re::mesh::{Mesh, PrimitiveTopology};
use re::system::{Pass, System};
use re::utils::bufferize_data;
use re::window::Window;
use re::{render_passes, Format, Image};

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
    let patched_shadow_image: Image = vulkano::image::AttachmentImage::sampled(device.clone(), SHADOW_MAP_DIMS, Format::D32Sfloat).unwrap();
    let mut depth_view_custom_images = HashMap::new();
    depth_view_custom_images.insert("patched_shadow", patched_shadow_image);

    let rpass1 = render_passes::only_depth(device.clone());
    let rpass2 = render_passes::with_depth(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![
            Pass {
                name: "geometry",
                images_created_tags: vec!["patched_shadow"],
                images_needed_tags: vec![],
                render_pass: rpass1.clone(),
                custom_images: HashMap::new(),
            },
            Pass {
                name: "depth_view",
                // also creates its own depth buffer
                images_created_tags: vec!["depth_view", "depth"],
                images_needed_tags: vec!["patched_shadow"],
                render_pass: rpass2.clone(),
                custom_images: depth_view_custom_images,
            },
        ],
        "depth_view",
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

    let mut dragon = ObjectPrototype {
        vs_path: relative_path("shaders/cubemap/vert.glsl"),
        fs_path: relative_path("shaders/cubemap/frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        mesh,
        custom_sets: vec![], // will be filled in later
        custom_dynamic_state: Some(custom_dynstate),
    }
    .into_renderable_object(queue.clone());

    let pipe_dragon = dragon
        .pipeline_spec
        .concrete(device.clone(), rpass1.clone());
    let model_set = pds_for_buffers(pipe_dragon.clone(), &[model_buffer], 0).unwrap();
    dragon.custom_sets = vec![model_set];

    // create cube to visualize cubemap on
    let cube_mesh = load_obj_positions_only(&relative_path("meshes/cube.obj"));
    let cube = ObjectPrototype {
        vs_path: relative_path("shaders/cubemap/depth_view_vert.glsl"),
        fs_path: relative_path("shaders/cubemap/depth_view_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        mesh: cube_mesh,
        custom_sets: vec![],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    let pipe_cube = cube
        .pipeline_spec
        .concrete(device.clone(), rpass2.clone());

    // used in main loop
    let mut all_objects = HashMap::new();

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());

        let camera_buffer = camera.get_buffer(queue.clone());
        let camera_set_dragon = pds_for_buffers(pipe_dragon.clone(), &[camera_buffer.clone()], 1).unwrap();
        let camera_set_cube = pds_for_buffers(pipe_cube.clone(), &[camera_buffer.clone()], 1).unwrap();

        // add camera set to both passes
        let mut cur_dragon = dragon.clone();
        cur_dragon.custom_sets.push(camera_set_dragon);
        let mut cur_cube = cube.clone();
        cur_cube.custom_sets.push(camera_set_cube);

        // replace old "geometry" object list
        all_objects.insert("geometry", vec![cur_dragon]);
        all_objects.insert("depth_view", vec![cur_cube]);

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

#[derive(Default, Debug, Clone, Copy)]
struct V3D {
    position: [f32; 3],
}
vulkano::impl_vertex!(V3D, position);

fn load_obj_positions_only(path: &Path) -> Mesh<V3D> {
    // loads the first mesh in an obj file, extracting only position information
    let (models, _materials) = tobj::load_obj(path).expect("Couldn't load OBJ file");

    // only use first mesh
    let mesh = &models[0].mesh;
    let mut vertices: Vec<V3D> = vec![];

    for i in 0..mesh.positions.len() / 3 {
        let pos = [
            mesh.positions[i * 3],
            mesh.positions[i * 3 + 1],
            mesh.positions[i * 3 + 2],
        ];
        let vertex = V3D {
            position: pos,
        };

        vertices.push(vertex);
    }

    println!("Vertices: {}", vertices.len());
    println!("Indices: {}", mesh.indices.len());

    Mesh {
        vertices,
        indices: mesh.indices.clone(),
    }
}
