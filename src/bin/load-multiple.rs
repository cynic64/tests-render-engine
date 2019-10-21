use render_engine as re;

use re::mesh::Mesh;

use tobj;

use re::collection_cache::pds_for_buffers;
use re::mesh::ObjectSpec;
use re::mesh::VertexType;
use re::render_passes;
use re::system::{Pass, System, RenderableObject};
use re::utils::bufferize_data;
use re::window::Window;
use re::pipeline_cache::PipelineSpec;
use re::PrimitiveTopology;

use vulkano::device::Queue;
use vulkano::framebuffer::RenderPassAbstract;

use nalgebra_glm as glm;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::marker::PhantomData;

use tests_render_engine::mesh::PosTexNorm;
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
            images_created_tags: vec!["resolve_color", "multisampled_color", "multisampled_depth", "resolve_depth"],
            images_needed_tags: vec![],
            render_pass: render_pass.clone(),
        }],
        "resolve_color",
    );

    window.set_render_pass(render_pass.clone());

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load objects
    let mut objects = load_objects(queue.clone(), render_pass.clone(), &relative_path("meshes/dodge.obj"));
    println!("Loaded: {}", objects.len());
    let mut all_objects = HashMap::new();

    // used in main loop
    let pipeline = objects[0].pipeline_spec.concrete(device.clone(), render_pass.clone());

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let camera_set =
            pds_for_buffers(pipeline.clone(), &[camera_buffer], 0).unwrap(); // 0 is the descriptor set idx
        objects.iter_mut().for_each(|obj| {
            // when first loaded, the objects are given a set for the model but
            // not the camera. if this is the case, we append the camera set to
            // that object. otherwise, overwrite the old camera set (which is at
            // idx 1)
            if obj.custom_sets.len() == 1 {
                obj.custom_sets.push(camera_set.clone());
            } else if obj.custom_sets.len() == 2 {
                obj.custom_sets[1] = camera_set.clone();
            } else {
                panic!("what the fuck is going on?");
            };
        });

        all_objects.insert("geometry", objects.clone());

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

fn load_objects(queue: Arc<Queue>, render_pass: Arc<dyn RenderPassAbstract + Send + Sync>, path: &Path) -> Vec<RenderableObject> {
    // create buffer for model matrix, used for all
    let model_data: [[f32; 4]; 4] = glm::Mat4::identity().into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // create concrete pipeline, used to create descriptor sets for all_objects
    let vtype = VertexType {
        phantom: PhantomData::<PosTexNorm>,
    };
    let pipeline_spec = PipelineSpec {
        vs_path: relative_path("shaders/load-multiple/basic_vert.glsl"),
        fs_path: relative_path("shaders/load-multiple/basic_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        depth: true,
        vtype: Arc::new(vtype),
    };
    let pipeline = pipeline_spec.concrete(queue.device().clone(), render_pass);
    let model_pds = pds_for_buffers(pipeline, &[model_buffer], 0).unwrap();

    // load
    let obj = tobj::load_obj(path).unwrap();
    let raw_meshes: Vec<tobj::Mesh> = obj.0.iter().map(|model| model.mesh.clone()).collect();
    let raw_materials = obj.1;
    println!("{} Materials", raw_materials.len());
    let meshes: Vec<Mesh<PosTexNorm>> = raw_meshes.iter().map(|mesh| convert_mesh(mesh)).collect();

    // process
    meshes.iter().map(|mesh| ObjectSpec {
        vs_path: relative_path("shaders/load-multiple/basic_vert.glsl"),
        fs_path: relative_path("shaders/load-multiple/basic_frag.glsl"),
        mesh: mesh.clone(),
        depth_buffer: true,
        custom_sets: vec![model_pds.clone()],
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

#[allow(dead_code)]
struct Material {
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
    shininess: f32,
}
