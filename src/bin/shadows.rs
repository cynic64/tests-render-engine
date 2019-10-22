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
    let render_pass = render_passes::multisampled_with_depth(device.clone(), 4);
    let shadow_rpass = render_passes::only_depth(device.clone());
    let final_rpass = render_passes::basic(device.clone());
    let mut system = System::new(
        queue.clone(),
        vec![
            Pass {
                name: "shadow",
                images_created_tags: vec!["shadow_depth"],
                images_needed_tags: vec![],
                render_pass: shadow_rpass.clone(),
            },
            Pass {
                name: "final",
                images_created_tags: vec!["final"],
                images_needed_tags: vec!["shadow_depth"],
                render_pass: final_rpass.clone(),
            },
            Pass {
                name: "geometry",
                images_created_tags: vec![
                    "resolve_color",
                    "multisampled_color",
                    "multisampled_depth",
                    "resolve_depth",
                ],
                images_needed_tags: vec!["shadow_depth"],
                render_pass: render_pass.clone(),
            },
        ],
        "resolve_color",
    );

    window.set_render_pass(shadow_rpass.clone());

    // initialize camera
    let mut camera = FlyCamera::default();

    // light
    let mut light = MovingLight::new();

    // load objects
    let objects = load_objects(
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
    let shadow_pipe_spec = PipelineSpec {
        vs_path: relative_path("shaders/shadows/light_vert.glsl"),
        fs_path: relative_path("shaders/shadows/light_frag.glsl"),
        vtype: VertexType::<PosTexNormTan>::new(),
        depth: true,
        ..Default::default()
    };
    let shadow_pipe = shadow_pipe_spec.concrete(device.clone(), shadow_rpass.clone());

    let final_pipe_spec = PipelineSpec {
        vs_path: relative_path("shaders/shadows/temp_vert.glsl"),
        fs_path: relative_path("shaders/shadows/temp_frag.glsl"),
        vtype: VertexType::<V2D>::new(),
        fill_type: PrimitiveTopology::TriangleStrip,
        ..Default::default()
    };
    let final_pipeline = final_pipe_spec.concrete(device.clone(), final_rpass.clone());
    let final_tri = RenderableObjectSpec {
        pipeline_spec: final_pipe_spec,
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

    all_objects.insert("final", vec![final_tri]);

    // this model buffer is used for everything in the shadow mapping pass,
    // which isn't great
    let light_model_mat: [[f32; 4]; 4] =
        glm::scale(&glm::Mat4::identity(), &glm::vec3(0.1, 0.1, 0.1)).into();
    let light_model_buffer = bufferize_data(queue.clone(), light_model_mat);
    let light_model_set = pds_for_buffers(shadow_pipe.clone(), &[light_model_buffer], 0).unwrap();
    let mut debug = false;

    while !window.update() {
        // update camera and light
        camera.update(window.get_frame_info());
        light.update();
        let camera_buffer = camera.get_buffer(queue.clone());

        let light_buffer = light.get_buffer(queue.clone());
        let camera_light_set =
            pds_for_buffers(pipeline.clone(), &[camera_buffer, light_buffer], 3).unwrap(); // 0 is the descriptor set idx
        let light_matrix_buffer = light.get_lightspace_matrix(queue.clone());
        let light_matrix_set =
            pds_for_buffers(shadow_pipe.clone(), &[light_matrix_buffer], 1).unwrap();

        all_objects.insert(
            "geometry",
            objects
                .clone()
                .iter_mut()
                .map(|obj| {
                    // add some sets to each object before adding it to the scene
                    obj.custom_sets.push(camera_light_set.clone());
                    obj.custom_sets.push(light_matrix_set.clone());
                    obj.clone()
                })
                .collect(),
        );

        all_objects.insert(
            "shadow",
            objects
                .clone()
                .iter_mut()
                .map(|obj| {
                    // replace textures with light matrix
                    obj.pipeline_spec = shadow_pipe_spec.clone();
                    obj.custom_sets = vec![light_model_set.clone(), light_matrix_set.clone()];
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
                system.output_tag = "final";
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
        vs_path: relative_path("shaders/shadows/object_vert.glsl"),
        fs_path: relative_path("shaders/shadows/object_frag.glsl"),
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
    position: [f32; 4],
}

struct LightMatrix {
    matrix: [[f32; 4]; 4],
}

impl MovingLight {
    fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            position: [0.0, 0.0, 0.0, 0.0],
        }
    }

    fn update(&mut self) {
        let time = get_elapsed(self.start_time);
        let time = 10.0f32;
        let distance = 50.0;
        self.position = [
            (time / 4.0).sin() * distance,
            (time / 8.0).sin() * distance * 0.2 + 10.0,
            (time / 3.0).sin() * distance,
            0.0,
        ];
        dbg![self.position];
        self.position = [0.0, 10.0, 10.0, 0.0];
    }

    fn get_buffer(&self, queue: Arc<Queue>) -> Arc<dyn BufferAccess + Send + Sync> {
        bufferize_data(
            queue.clone(),
            Light {
                position: self.position,
                power: 2.0,
            },
        )
    }

    fn get_lightspace_matrix(&self, queue: Arc<Queue>) -> Arc<dyn BufferAccess + Send + Sync> {
        use glm::*;
        let up = vec3(0.0, 1.0, 0.0);

        let view: Mat4 = look_at(
            &vec3(self.position[0], self.position[1], self.position[2]),
            &vec3(0.0, 10.0, 0.0),
            &up,
        )
        .into();

        let proj: Mat4 = scale(
            &perspective(
                // aspect ratio
                16.0 / 9.0,
                // fov
                2.0,
                // near
                0.1,
                // far
                1_000.,
            ),
            &vec3(1.0, -1.0, 1.0),
        )
            .into();
        // let proj: Mat4 = ortho_zo(-10.0, 10.0, 10.0, -10.0, 0.1, 1_000.0);

        bufferize_data(
            queue.clone(),
            LightMatrix {
                matrix: (proj * view).into(),
            },
        )
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct V2D {
    tex_coords: [f32; 2],
}
vulkano::impl_vertex!(V2D, tex_coords);
