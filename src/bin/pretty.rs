use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::input::{get_elapsed, VirtualKeyCode};
use re::mesh::{ObjectPrototype, PrimitiveTopology, VertexType};
use re::pipeline_cache::PipelineSpec;
use re::render_passes;
use re::system::{Pass, RenderableObject, System};
use re::utils::{bufferize_data, Timer};
use re::window::Window;
use re::{Buffer, Format, Image, Pipeline, Queue, Set};

use vulkano::command_buffer::DynamicState;
use vulkano::pipeline::viewport::Viewport;

use std::collections::HashMap;

use nalgebra_glm::*;

use tests_render_engine::mesh::{
    convert_mesh, fullscreen_quad, load_obj, load_obj_single, merge, Vertex3D,
};
use tests_render_engine::{relative_path, FlyCamera};

const SHADOW_MAP_DIMS: [u32; 2] = [6_144, 1024];
const PATCH_DIMS: [f32; 2] = [1024.0, 1024.0];

/*
const SHADOW_MAP_DIMS: [u32; 2] = [3_072, 512];
const PATCH_DIMS: [f32; 2] = [512.0, 512.0];
*/

fn main() {
    // initialize window
    let (mut window, queue) = Window::new();
    let device = queue.device().clone();

    // create system
    let patched_shadow: Image = vulkano::image::AttachmentImage::sampled(
        device.clone(),
        SHADOW_MAP_DIMS,
        Format::D32Sfloat,
    )
    .unwrap();
    let shadow_blur: Image = vulkano::image::AttachmentImage::sampled(
        device.clone(),
        // SHADOW_MAP_DIMS,
        SHADOW_MAP_DIMS,
        Format::D32Sfloat,
    )
    .unwrap();
    let mut custom_images = HashMap::new();
    custom_images.insert("shadow_map", patched_shadow);
    custom_images.insert("shadow_map_blur", shadow_blur);

    let render_pass = render_passes::read_depth(device.clone());
    let rpass_shadow = render_passes::only_depth(device.clone());
    let rpass_shadow_blur = render_passes::only_depth(device.clone());
    let rpass_cubeview = render_passes::basic(device.clone());
    let rpass_prepass = render_passes::only_depth(device.clone());

    let mut system = System::new(
        queue.clone(),
        vec![
            // renders to shadow cubemap
            Pass {
                name: "shadow",
                images_created_tags: vec!["shadow_map"],
                images_needed_tags: vec![],
                render_pass: rpass_shadow.clone(),
            },
            // blurs shadow cubemap
            Pass {
                name: "shadow_blur",
                images_created_tags: vec!["shadow_map_blur"],
                images_needed_tags: vec!["shadow_map"],
                render_pass: rpass_shadow_blur.clone(),
            },
            // depth prepass
            Pass {
                name: "depth_prepass",
                images_created_tags: vec!["depth_prepass"],
                images_needed_tags: vec![],
                render_pass: rpass_prepass.clone(),
            },
            // displays any depth buffer for debugging
            Pass {
                name: "depth_viewer",
                images_created_tags: vec!["depth_view"],
                // images_needed_tags: vec!["shadow_map_blur"],
                images_needed_tags: vec!["depth_prepass"],
                render_pass: rpass_cubeview.clone(),
            },
            // final pass
            Pass {
                name: "geometry",
                images_created_tags: vec!["color", "depth_prepass"],
                images_needed_tags: vec!["shadow_map_blur"],
                render_pass: render_pass.clone(),
            },
        ],
        custom_images,
        "color",
    );

    window.set_render_pass(render_pass.clone());

    // initialize camera
    let mut camera = FlyCamera::default();
    camera.yaw = 0.0;
    camera.position = vec3(0.0, 10.0, 0.0);

    // light
    let light = MovingLight::new();

    // a model buffer with .1 scale, used for a couple different objects
    let model_data: [[f32; 4]; 4] = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // load objects
    let mut objects = load_obj(
        queue.clone(),
        render_pass.clone(),
        &relative_path("meshes/sponza/sponza.obj"),
        relative_path("shaders/pretty/vert.glsl"),
        relative_path("shaders/pretty/all_frag.glsl"),
        2, // textures set idx
    );
    println!("Objects Loaded: {}", objects.len());

    let pipeline = objects[0]
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());

    // load sphere to show light's position
    let sphere_mesh = load_obj_single(&relative_path("meshes/sphere.obj"));
    let mut sphere = ObjectPrototype {
        vs_path: relative_path("shaders/pretty/light_vert.glsl"),
        fs_path: relative_path("shaders/pretty/light_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        // merge converts to Vertex3D which is what we want
        mesh: merge(&[sphere_mesh]),
        custom_sets: vec![],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());
    let pipe_light = sphere
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let model_set = pds_for_buffers(pipe_light.clone(), &[model_buffer.clone()], 1).unwrap();
    sphere.custom_sets = vec![model_set];

    // shadow stuff
    // create fullscreen quad to debug cubemap
    let quad_display = fullscreen_quad(
        queue.clone(),
        relative_path("shaders/pretty/fullscreen_vert.glsl"),
        relative_path("shaders/pretty/display_cubemap_frag.glsl"),
    );

    // and to blur shadow map
    let mut quad_blur = fullscreen_quad(
        queue.clone(),
        relative_path("shaders/pretty/fullscreen_vert.glsl"),
        relative_path("shaders/pretty/blur_frag.glsl"),
    );
    quad_blur.pipeline_spec.write_depth = true;

    let pipe_spec_caster = PipelineSpec {
        vs_path: relative_path("shaders/pretty/shadow_cast_vert.glsl"),
        fs_path: relative_path("shaders/pretty/shadow_cast_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        vtype: VertexType::<Vertex3D>::new(),
    };
    let pipe_caster = pipe_spec_caster.concrete(device.clone(), rpass_shadow.clone());

    let pipe_spec_prepass = PipelineSpec {
        vs_path: relative_path("shaders/pretty/depth_prepass_vert.glsl"),
        fs_path: relative_path("shaders/pretty/depth_prepass_frag.glsl"),
        ..pipe_spec_caster.clone()
    };
    let pipe_prepass = pipe_spec_prepass.concrete(device.clone(), rpass_prepass.clone());

    // concatenate all meshes
    // we load them a second time, which i'd like to change at some point
    let meshes: Vec<_> = tobj::load_obj(&relative_path("meshes/sponza/sponza.obj"))
        .unwrap()
        .0
        .iter()
        .map(|model| convert_mesh(&model.mesh))
        .collect();
    let merged_mesh = merge(&meshes);
    let model_set = pds_for_buffers(pipe_caster.clone(), &[model_buffer], 0).unwrap();
    let merged_object = ObjectPrototype {
        vs_path: relative_path("shaders/pretty/shadow_cast_vert.glsl"),
        fs_path: relative_path("shaders/pretty/shadow_cast_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        mesh: merged_mesh,
        // copy the model matrix from the first object that we loaded earlier
        custom_sets: vec![model_set],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    let mut all_objects = HashMap::new();

    // used in main loop
    let mut timer_setup = Timer::new("Setup time");
    let mut timer_draw = Timer::new("Overall draw time");

    all_objects.insert("depth_viewer", vec![quad_display]);
    all_objects.insert("shadow_blur", vec![quad_blur]);

    let mut view_mode = 0;

    while !window.update() {
        timer_setup.start();

        // convert merged mesh into 6 casters, one for each cubemap face
        let shadow_casters = convert_to_shadow_casters(
            queue.clone(),
            pipe_caster.clone(),
            merged_object.clone(),
            light.get_position(),
        );
        // update camera and light
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let light_buffer = light.get_buffer(queue.clone());
        // used for color pass
        let camera_light_set = pds_for_buffers(
            pipeline.clone(),
            &[camera_buffer.clone(), light_buffer.clone()],
            3,
        )
        .unwrap(); // 0 is the descriptor set idx
                   // used for shadow pass
        let light_set = pds_for_buffers(pipe_caster.clone(), &[light_buffer], 3).unwrap();
        let camera_set = pds_for_buffers(pipe_prepass.clone(), &[camera_buffer], 1).unwrap();

        let mut depth_prepass_object = merged_object.clone();
        depth_prepass_object.custom_sets.push(camera_set.clone());
        depth_prepass_object.pipeline_spec = pipe_spec_prepass.clone();
        let mut light_object = sphere.clone();
        let light_model_matrix = scale(
            &translate(&Mat4::identity(), &make_vec3(&light.get_position())),
            &vec3(0.03, 0.03, 0.03),
        );
        let light_model_buffer = bufferize_data(queue.clone(), light_model_matrix);
        let prepass_light_model_set =
            pds_for_buffers(pipe_prepass.clone(), &[light_model_buffer.clone()], 0).unwrap();
        light_object.custom_sets = vec![prepass_light_model_set, camera_set.clone()];
        light_object.pipeline_spec = pipe_spec_prepass.clone();
        all_objects.insert("depth_prepass", vec![depth_prepass_object, light_object]);

        if window.get_frame_info().keydowns.contains(&VirtualKeyCode::C) {
            view_mode += 1;

            match view_mode {
                0 => {
                    // default: everything enabled
                    system.output_tag = "color";
                },
                1 => {
                    // pur white
                    objects.iter_mut().for_each(|obj| {
                        obj.pipeline_spec.fs_path = relative_path("shaders/pretty/white_frag.glsl");
                    });
                    system.output_tag = "color";
                },
                2 => {
                    // depth only
                    system.output_tag = "depth_view";
                },
                _ => {
                    objects.iter_mut().for_each(|obj| {
                        obj.pipeline_spec.fs_path = relative_path("shaders/pretty/all_frag.glsl");
                    });
                    view_mode = 0;
                    system.output_tag = "color";
                },
            }
        }

        let geometry_light_model_set =
            pds_for_buffers(pipe_light.clone(), &[light_model_buffer.clone()], 1).unwrap();

        all_objects.insert(
            "geometry",
            objects
                .clone()
                .iter_mut()
                .map(|obj| {
                    obj.pipeline_spec.write_depth = false;
                    obj.pipeline_spec.read_depth = true;
                    // add camera set to each object before adding it to the scene
                    obj.custom_sets.push(camera_light_set.clone());
                    obj.clone()
                })
                .chain(vec![{
                    let mut cur_sphere = sphere.clone();
                    cur_sphere.custom_sets = vec![geometry_light_model_set, camera_set.clone()];
                    cur_sphere.clone()
                }])
                .collect(),
        );

        all_objects.insert(
            "shadow",
            shadow_casters
                .clone()
                .iter_mut()
                .map(|obj| {
                    // add camera set to each object before adding it to the scene
                    obj.custom_sets.push(light_set.clone());
                    obj.clone()
                })
                .collect(),
        );

        timer_setup.stop();

        // draw
        timer_draw.start();
        system.render_to_window(&mut window, all_objects.clone());
        timer_draw.stop();
    }

    system.print_stats();
    println!("FPS: {}", window.get_fps());
    println!("Avg. delta: {} ms", window.get_avg_delta() * 1_000.0);
    timer_setup.print();
    timer_draw.print();
}

#[allow(dead_code)]
struct Light {
    position: [f32; 4],
    strength: f32,
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

    fn get_buffer(&self, queue: Queue) -> Buffer {
        let time = get_elapsed(self.start_time) / 16.0;
        let data = Light {
            // position: [time.sin(), 10.0, time.cos(), 0.0],
            position: [time.sin() * 100.0, 10.0, 0.0, 0.0],
            strength: 1.0,
        };

        bufferize_data(queue.clone(), data)
    }

    fn get_position(&self) -> [f32; 3] {
        let time = get_elapsed(self.start_time) / 16.0;
        [time.sin() * 100.0, 10.0, 0.0]
    }
}

fn convert_to_shadow_casters(
    queue: Queue,
    pipeline: Pipeline,
    base_object: RenderableObject,
    light_pos: [f32; 3],
) -> Vec<RenderableObject> {
    // if you want to make point lamps cast shadows, you need shadow cubemaps
    // render-engine doesn't support geometry shaders, so the easiest way to do
    // this is to convert one object into 6 different ones, one for each face of
    // the cubemap, that each render to a different part of a 2D texture.
    // for now this function assumes a 6x1 patch layout
    let view_directions = [
        vec3(1.0, 0.0, 0.0),
        vec3(-1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, -1.0, 0.0),
        vec3(0.0, 0.0, 1.0),
        vec3(0.0, 0.0, -1.0),
    ];

    let up_directions = [
        vec3(0.0, -1.0, 0.0),
        vec3(0.0, -1.0, 0.0),
        vec3(0.0, 0.0, 1.0),
        vec3(0.0, 0.0, -1.0),
        vec3(0.0, -1.0, 0.0),
        vec3(0.0, -1.0, 0.0),
    ];

    let patch_positions = [
        [0.0, 0.0],
        [1.0, 0.0],
        [2.0, 0.0],
        [3.0, 0.0],
        [4.0, 0.0],
        [5.0, 0.0],
    ];

    let proj_set = create_projection_set(queue.clone(), pipeline.clone());

    let model_data: [[f32; 4]; 4] = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);
    let model_set = pds_for_buffers(pipeline.clone(), &[model_buffer], 0).unwrap();

    let light_pos = make_vec3(&light_pos);

    view_directions
        .iter()
        .zip(&up_directions)
        .zip(&patch_positions)
        .map(|((dir, up), patch_pos): ((&Vec3, &Vec3), &[f32; 2])| {
            let view_matrix: [[f32; 4]; 4] = look_at(&light_pos, &(light_pos + dir), up).into();
            let view_buffer = bufferize_data(queue.clone(), view_matrix);
            let set = pds_for_buffers(pipeline.clone(), &[view_buffer], 2).unwrap();

            // all sets for the dragon we're currently creating
            // we take the model set from the base dragon
            // (set 0)
            let custom_sets = vec![model_set.clone(), proj_set.clone(), set];

            // dynamic state for the current dragon, represents which part
            // of the patched texture we draw to
            let margin = 0.0;
            let origin = [
                patch_pos[0] * PATCH_DIMS[0] + margin,
                patch_pos[1] * PATCH_DIMS[1] + margin,
            ];
            let dynamic_state = dynamic_state_for_bounds(
                origin,
                [PATCH_DIMS[0] - margin * 2.0, PATCH_DIMS[1] - margin * 2.0],
            );

            RenderableObject {
                // model and proj are in set 0 and 1
                custom_sets,
                custom_dynamic_state: Some(dynamic_state),
                ..base_object.clone()
            }
        })
        .collect()
}

fn create_projection_set(queue: Queue, pipeline: Pipeline) -> Set {
    let (near, far) = (1.0, 250.0);
    // pi / 2 = 90 deg., 1.0 = aspect ratio
    // we use a fov 1% too big to make sure sampling doesn't go between patches
    let proj_data: [[f32; 4]; 4] =
        perspective(1.0, std::f32::consts::PI / 2.0 * 1.01, near, far).into();
    let proj_buffer = bufferize_data(queue, proj_data);

    pds_for_buffers(pipeline, &[proj_buffer], 1).unwrap()
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
