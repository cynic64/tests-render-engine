use render_engine as re;

use re::mesh::Mesh;

use tobj;

use re::collection_cache::{pds_for_buffers, pds_for_images};
use re::mesh::ObjectSpec;
use re::mesh::VertexType;
use re::pipeline_cache::PipelineSpec;
use re::render_passes;
use re::system::{Pass, RenderableObject, System};
use re::utils::{bufferize_data, load_texture};
use re::window::Window;
use re::PrimitiveTopology;
use re::input::get_elapsed;

use vulkano::device::Queue;
use vulkano::framebuffer::RenderPassAbstract;
use vulkano::format::Format;

use nalgebra_glm as glm;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use tests_render_engine::mesh::{PosTexNorm, PosTexNormTan, add_tangents};
use tests_render_engine::{default_sampler, relative_path, FlyCamera};

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

    // load objects
    let mut objects = load_objects(
        queue.clone(),
        render_pass.clone(),
        &relative_path("meshes/sponza/sponza.obj"),
    );
    println!("Loaded: {}", objects.len());
    let mut all_objects = HashMap::new();

    // used in main loop
    let pipeline = objects[0]
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let camera_set = pds_for_buffers(pipeline.clone(), &[camera_buffer], 2).unwrap(); // 0 is the descriptor set idx
        objects.iter_mut().for_each(|obj| {
            // when first loaded, the objects are given a set for the model and
            // textures but not the camera. if this is the case, we append the
            // camera set to that object. otherwise, overwrite the old camera
            // set (which is at idx 2)
            if obj.custom_sets.len() == 2 {
                obj.custom_sets.push(camera_set.clone());
            } else if obj.custom_sets.len() == 3 {
                obj.custom_sets[2] = camera_set.clone();
            } else {
                panic!("wrong set count, noooo");
            };
        });

        all_objects.insert("geometry", objects.clone());

        // draw
        let st = std::time::Instant::now();
        system.render_to_window(&mut window, all_objects.clone());
        println!("grand total: {}ms", 1_000.0 * get_elapsed(st));
        println!();
    }

    println!("FPS: {}", window.get_fps());
}

fn load_objects(
    queue: Arc<Queue>,
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    path: &Path,
) -> Vec<RenderableObject> {
    // create buffer for model matrix, used for all
    let model_data: [[f32; 4]; 4] =
        glm::scale(&glm::Mat4::identity().into(), &glm::vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // create concrete pipeline, used to create descriptor sets for all_objects
    let vtype = VertexType {
        phantom: PhantomData::<PosTexNormTan>,
    };
    let pipeline_spec = PipelineSpec {
        vs_path: relative_path("shaders/load-multiple/basic_vert.glsl"),
        fs_path: relative_path("shaders/load-multiple/basic_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        depth: true,
        vtype: Arc::new(vtype),
    };
    let pipeline = pipeline_spec.concrete(queue.device().clone(), render_pass);

    // load
    let obj = tobj::load_obj(path).unwrap();
    let raw_meshes: Vec<tobj::Mesh> = obj.0.iter().map(|model| model.mesh.clone()).collect();
    let meshes: Vec<(Mesh<PosTexNormTan>, usize)> = raw_meshes
        .iter()
        .map(|mesh| (convert_mesh(mesh), mesh.material_id.unwrap_or(0)))
        .collect();

    // create material buffers and load textures
    let raw_materials = obj.1;
    let materials: Vec<_> = raw_materials
        .iter()
        .map(|mat| {
            bufferize_data(
                queue.clone(),
                Material {
                    ambient: [mat.ambient[0], mat.ambient[1], mat.ambient[2], 0.0],
                    diffuse: [mat.diffuse[0], mat.diffuse[1], mat.diffuse[2], 0.0],
                    specular: [mat.specular[0], mat.specular[1], mat.specular[2], 0.0],
                    shininess: mat.shininess,
                },
            )
        })
        .collect();

    let sampler = default_sampler(queue.device().clone());

    let textures: Vec<_> = raw_materials
        .iter()
        .map(|mat| {
            let diff_path = if mat.diffuse_texture == "" {
                relative_path("textures/missing.png")
            } else {
                relative_path(&format!("meshes/sponza/{}", mat.diffuse_texture))
            };

            let spec_path = if mat.specular_texture == "" {
                relative_path("textures/missing-spec.png")
            } else {
                relative_path(&format!("meshes/sponza/{}", mat.specular_texture))
            };

            let normal_path = if mat.normal_texture == "" {
                relative_path("textures/missing.png")
            } else {
                relative_path(&format!("meshes/sponza/{}", mat.normal_texture))
            };

            let diff_tex = load_texture(queue.clone(), &diff_path, Format::R8G8B8A8Srgb);
            let spec_tex = load_texture(queue.clone(), &spec_path, Format::R8G8B8A8Unorm);
            let norm_tex = load_texture(queue.clone(), &normal_path, Format::R8G8B8A8Unorm);
            pds_for_images(sampler.clone(), pipeline.clone(), &[diff_tex, spec_tex, norm_tex], 1).unwrap()
        })
        .collect();

    // process
    meshes
        .iter()
        .map(|(mesh, material_idx)| {
            ObjectSpec {
                vs_path: relative_path("shaders/load-multiple/basic_vert.glsl"),
                fs_path: relative_path("shaders/load-multiple/basic_frag.glsl"),
                mesh: mesh.clone(),
                depth_buffer: true,
                custom_sets: vec![
                    pds_for_buffers(
                        pipeline.clone(),
                        &[materials[*material_idx].clone(), model_buffer.clone()],
                        0,
                    )
                    .unwrap(),
                    textures[*material_idx].clone(),
                ],
                ..Default::default()
            }
            .build(queue.clone())
        })
        .collect()
}

fn convert_mesh(mesh: &tobj::Mesh) -> Mesh<PosTexNormTan> {
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
            [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1] * -1.0]
        };

        vertices.push(PosTexNorm {
            position,
            tex_coord,
            normal,
        });
    }

    let base_mesh = Mesh {
        vertices,
        indices: mesh.indices.clone(),
    };

    add_tangents(&base_mesh)
}

#[allow(dead_code)]
#[derive(Debug)]
struct Material {
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
    shininess: f32,
}
