use render_engine as re;

use re::mesh::Mesh;

use tobj;

use re::collection_cache::pds_for_buffers;
use re::mesh::ObjectSpec;
use re::render_passes;
use re::system::{Pass, System, RenderableObject};
use re::utils::bufferize_data;
use re::window::Window;

use vulkano::device::Queue;

use nalgebra_glm as glm;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tests_render_engine::mesh::PosTexNorm;
use tests_render_engine::{relative_path, OrbitCamera};

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
    let model_data: [[f32; 4]; 4] = glm::Mat4::identity().into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load objects
    let mut objects = load_objects(queue.clone(), &relative_path("meshes/dodge.obj"));
    println!("Loaded: {}", objects.len());
    let mut all_objects = HashMap::new();

    // used in main loop
    let pipeline = objects[0].pipeline_spec.concrete(device.clone(), render_pass.clone());

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let set =
            pds_for_buffers(pipeline.clone(), &[model_buffer.clone(), camera_buffer], 0).unwrap(); // 0 is the descriptor set idx
        objects.iter_mut().for_each(|obj| obj.custom_set = Some(set.clone()));

        all_objects.insert("geometry", objects.clone());

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

fn load_objects(queue: Arc<Queue>, path: &Path) -> Vec<RenderableObject> {
    let raw_meshes: Vec<tobj::Mesh> = tobj::load_obj(path).unwrap().0.iter().map(|model| model.mesh.clone()).collect();
    let meshes: Vec<Mesh<PosTexNorm>> = raw_meshes.iter().map(|mesh| convert_mesh(mesh)).collect();

    meshes.iter().map(|mesh| ObjectSpec {
        vs_path: relative_path("shaders/load-multiple/basic_vert.glsl"),
        fs_path: relative_path("shaders/load-multiple/basic_frag.glsl"),
        mesh: mesh.clone(),
        depth_buffer: true,
        ..Default::default()
    }.build(queue.clone())).collect()
}

fn convert_mesh(mesh: &tobj::Mesh) -> Mesh<PosTexNorm> {
    let mut vertices = vec![];
    for i in 0..mesh.positions.len() / 3 {
        let position = [
            mesh.positions[i * 3],
            mesh.positions[i * 3 + 1],
            mesh.positions[i * 3 + 2],
        ];
        let normal = [
            mesh.normals[i * 3],
            mesh.normals[i * 3 + 1],
            mesh.normals[i * 3 + 2],
        ];
        let tex_coord = if mesh.texcoords.len() <= i * 2 + 1 {
            [0.0, 0.0]
        } else {
            [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]]
        };

        vertices.push(PosTexNorm {
            position,
            tex_coord,
            normal,
        });
    }

    Mesh {
        vertices,
        indices: mesh.indices.clone(),
    }
}
