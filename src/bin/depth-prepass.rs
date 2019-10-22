use render_engine as re;

use re::mesh::Mesh;

use tobj;

use re::collection_cache::{pds_for_buffers, pds_for_images};
use re::input::get_elapsed;
use re::mesh::RenderableObjectSpec;
use re::mesh::VertexType;
use re::pipeline_cache::PipelineSpec;
use re::render_passes;
use re::system::{Pass, RenderableObject, System};
use re::utils::{bufferize_data, load_texture};
use re::window::Window;
use re::PrimitiveTopology;
use re::input::VirtualKeyCode;

use vulkano::buffer::BufferAccess;
use vulkano::device::Queue;
use vulkano::format::Format;
use vulkano::framebuffer::RenderPassAbstract;

use nalgebra_glm as glm;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use tests_render_engine::mesh::{add_tangents, PosTexNorm, PosTexNormTan};
use tests_render_engine::{default_sampler, relative_path, FlyCamera};

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let render_pass = render_passes::multisampled(device.clone(), 4);
    let depth_rpass = render_passes::only_depth(device.clone());
    let depth_view_rpass = render_passes::basic(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![
            Pass {
                name: "depth",
                images_created_tags: vec!["depth"],
                images_needed_tags: vec![],
                render_pass: depth_rpass.clone(),
            },
            Pass {
                name: "geometry",
                images_created_tags: vec![
                    "resolve_color",
                    "multisampled_color",
                ],
                images_needed_tags: vec!["depth"],
                render_pass: render_pass.clone(),
            },
            Pass {
                name: "depth_view",
                images_created_tags: vec!["depth_view"],
                images_needed_tags: vec!["depth"],
                render_pass: depth_view_rpass.clone(),
            },
        ],
        "resolve_color",
    );

    window.set_render_pass(depth_view_rpass.clone());

    // initialize camera
    let mut camera = FlyCamera::default();

    // light
    let light = MovingLight::new();

    // create fullscreen quad
    let depth_pipe_spec = PipelineSpec {
        vs_path: relative_path("shaders/depth-prepass/depth_vert.glsl"),
        fs_path: relative_path("shaders/depth-prepass/depth_frag.glsl"),
        vtype: VertexType::<PosTexNormTan>::new(),
        fill_type: PrimitiveTopology::TriangleList,
        depth: true,
    };
    let fullscreen_pipe_spec = PipelineSpec {
        vs_path: relative_path("shaders/depth-prepass/convert_vert.glsl"),
        fs_path: relative_path("shaders/depth-prepass/convert_frag.glsl"),
        vtype: VertexType::<V2D>::new(),
        fill_type: PrimitiveTopology::TriangleStrip,
        depth: false,
    };
    let fullscreen_quad = RenderableObjectSpec {
        pipeline_spec: fullscreen_pipe_spec.clone(),
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
        ..Default::default()
    }
    .build(queue.clone());

    // load objects
    let objects = load_objects(
        queue.clone(),
        render_pass.clone(),
        &relative_path("meshes/sponza/sponza.obj"),
    );
    println!("Loaded: {}", objects.len());
    let mut all_objects = HashMap::new();
    all_objects.insert("depth_view", vec![fullscreen_quad]);

    // used in main loop
    let fullscreen_pipeline = fullscreen_pipe_spec.concrete(device.clone(), depth_view_rpass.clone());
    let depth_pipeline = depth_pipe_spec.concrete(device.clone(), depth_rpass.clone());

    let model_mat: [[f32; 4]; 4] = glm::scale(&glm::Mat4::identity(), &glm::vec3(0.1, 0.1, 0.1)).into();
    let model_buf = bufferize_data(queue.clone(), model_mat);
    let model_set = pds_for_buffers(depth_pipeline.clone(), &[model_buf], 0).unwrap();

    let mut debug = false;

    while !window.update() {
        // update camera and light
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let light_buffer = light.get_buffer(queue.clone());
        let camera_light_set =
            pds_for_buffers(depth_pipeline.clone(), &[camera_buffer.clone(), light_buffer.clone()], 1).unwrap(); // 0 is the descriptor set idx
        // let camera_light_set_for_objects =
        //     pds_for_buffers(pipeline.clone(), &[camera_buffer, light_buffer], 3).unwrap(); // 0 is the descriptor set idx

        all_objects.insert(
            "depth",
            objects
                .clone()
                .iter_mut()
                .map(|obj| {
                    // add camera set to each object before adding it to the scene
                    obj.pipeline_spec = PipelineSpec {
                        vs_path: relative_path("shaders/depth-prepass/depth_vert.glsl"),
                        fs_path: relative_path("shaders/depth-prepass/depth_frag.glsl"),
                        fill_type: PrimitiveTopology::TriangleList,
                        depth: true,
                        vtype: VertexType::<PosTexNormTan>::new(),
                    };
                    obj.custom_sets = vec![model_set.clone(), camera_light_set.clone()];
                    obj.clone()
                })
                .collect(),
        );

        all_objects.insert(
            "geometry",
            objects
                .clone()
                .iter_mut()
                .map(|obj| {
                    // add camera set to each object before adding it to the scene
                    obj.custom_sets.push(camera_light_set.clone());
                    obj.clone()
                })
                .collect(),
        );

        if window
            .get_frame_info()
            .keydowns
            .contains(&VirtualKeyCode::C)
        {
            debug = !debug;
            if debug {
                system.output_tag = "depth_view";
            } else {
                system.output_tag = "resolve_color";
            }
        }

        // draw
        system.render_to_window(&mut window, all_objects.clone());
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
        vs_path: relative_path("shaders/depth-prepass/object_vert.glsl"),
        fs_path: relative_path("shaders/depth-prepass/object_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        depth: false,
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
            pds_for_images(
                sampler.clone(),
                pipeline.clone(),
                &[diff_tex, spec_tex, norm_tex],
                2,
            )
            .unwrap()
        })
        .collect();

    // process
    meshes
        .iter()
        .map(|(mesh, material_idx)| {
            RenderableObjectSpec {
                pipeline_spec: pipeline_spec.clone(),
                mesh: mesh.clone(),
                custom_sets: vec![
                    pds_for_buffers(
                        pipeline.clone(),
                        &[materials[*material_idx].clone(), model_buffer.clone()],
                        1,
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
struct Material {
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
    shininess: f32,
}

#[allow(dead_code)]
struct Light {
    position: [f32; 4],
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

    fn get_buffer(&self, queue: Arc<Queue>) -> Arc<dyn BufferAccess + Send + Sync> {
        let time = get_elapsed(self.start_time);
        let distance = 50.0;
        bufferize_data(
            queue.clone(),
            Light {
                position: [
                    (time / 4.0).sin() * distance,
                    (time / 8.0).sin() * distance,
                    (time / 3.0).sin() * distance,
                    0.0,
                ],
                power: 2.0,
            },
        )
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct V2D {
    tex_coords: [f32; 2],
}
vulkano::impl_vertex!(V2D, tex_coords);
