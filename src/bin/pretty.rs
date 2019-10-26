use render_engine as re;

use re::collection_cache::pds_for_buffers;
use re::mesh::{ObjectPrototype, PrimitiveTopology, VertexType};
use re::pipeline_cache::PipelineSpec;
use re::render_passes;
use re::system::{Pass, RenderableObject, System};
use re::utils::{bufferize_data, Timer};
use re::window::Window;
use re::{Buffer, Format, Image, Pipeline, Queue, Set};
use re::input::get_elapsed;

use vulkano::command_buffer::DynamicState;
use vulkano::pipeline::viewport::Viewport;

use std::collections::HashMap;

use nalgebra_glm::*;

use tests_render_engine::mesh::{convert_mesh, fullscreen_quad, load_obj, merge, Vertex3D};
use tests_render_engine::{relative_path, FlyCamera};

/*
const SHADOW_MAP_DIMS: [u32; 2] = [12_288, 2048];
const PATCH_DIMS: [f32; 2] = [2048.0, 2048.0];
*/

const SHADOW_MAP_DIMS: [u32; 2] = [3072, 512];
const PATCH_DIMS: [f32; 2] = [512.0, 512.0];

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
    let mut custom_images = HashMap::new();
    custom_images.insert("shadow_map", patched_shadow_image);

    let render_pass = render_passes::multisampled_with_depth(device.clone(), 4);
    let rpass_shadow = render_passes::only_depth(device.clone());
    let rpass_cubeview = render_passes::basic(device.clone());

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
            // displays shadow map for debugging
            Pass {
                name: "cubemap_view",
                images_created_tags: vec!["cubemap_view"],
                images_needed_tags: vec!["shadow_map"],
                render_pass: rpass_cubeview.clone(),
            },
            Pass {
                name: "geometry",
                images_created_tags: vec![
                    "resolve_color",
                    "multisampled_color",
                    "multisampled_depth",
                    "resolve_depth",
                ],
                images_needed_tags: vec!["shadow_map"],
                render_pass: render_pass.clone(),
            },
        ],
        custom_images,
        "resolve_color",
    );

    window.set_render_pass(render_pass.clone());

    // initialize camera
    let mut camera = FlyCamera::default();

    // light
    let light = MovingLight::new();

    // load objects
    let objects = load_obj(
        queue.clone(),
        render_pass.clone(),
        &relative_path("meshes/sponza/sponza.obj"),
        relative_path("shaders/pretty/vert.glsl"),
        relative_path("shaders/pretty/all_frag.glsl"),
    );
    println!("Objects Loaded: {}", objects.len());

    // shadow stuff
    // create fullscreen quad to debug cubemap
    let quad = fullscreen_quad(
        queue.clone(),
        relative_path("shaders/pretty/display_cubemap_vert.glsl"),
        relative_path("shaders/pretty/display_cubemap_frag.glsl"),
    );

    let pipe_spec_caster = PipelineSpec {
        vs_path: relative_path("shaders/pretty/shadow_cast_vert.glsl"),
        fs_path: relative_path("shaders/pretty/shadow_cast_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        vtype: VertexType::<Vertex3D>::new(),
    };
    let pipe_caster = pipe_spec_caster.concrete(device.clone(), rpass_shadow.clone());

    // concatenate all meshes
    // we load them a second time, which i'd like to change at some point
    let meshes: Vec<_> = tobj::load_obj(&relative_path("meshes/sponza/sponza.obj"))
        .unwrap()
        .0
        .iter()
        .map(|model| convert_mesh(&model.mesh))
        .collect();
    let merged_mesh = merge(&meshes);
    let model_data: [[f32; 4]; 4] = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);
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
    let pipeline = objects[0]
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());

    let mut timer_setup = Timer::new("Setup time");
    let mut timer_draw = Timer::new("Overall draw time");

    all_objects.insert("cubemap_view", vec![quad]);

    while !window.update() {
        timer_setup.start();

        // convert merged mesh into 6 casters, one for each cubemap face
        let shadow_casters =
            convert_to_shadow_casters(queue.clone(), pipe_caster.clone(), merged_object.clone(), light.get_position());
        // update camera and light
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let light_buffer = light.get_buffer(queue.clone());
        // used for color pass
        let camera_light_set =
            pds_for_buffers(pipeline.clone(), &[camera_buffer, light_buffer.clone()], 3).unwrap(); // 0 is the descriptor set idx
        // used for shadow pass
        let light_set = pds_for_buffers(pipe_caster.clone(), &[light_buffer], 3).unwrap();

        system.output_tag = if window.get_frame_info().keys_down.c {
            "cubemap_view"
        } else {
            "resolve_color"
        };

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
            strength: 2.0,
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

fn create_projection_set(queue: Queue, pipeline: Pipeline) -> Set {
    let (near, far) = (1.0, 250.0);
    // pi / 2 = 90 deg., 1.0 = aspect ratio
    let proj_data: [[f32; 4]; 4] = perspective(1.0, std::f32::consts::PI / 2.0, near, far).into();
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
