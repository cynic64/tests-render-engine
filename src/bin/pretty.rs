use render_engine as re;

use re::{Queue, Buffer, Image, Format, Pipeline, Set};
use re::collection_cache::pds_for_buffers;
use re::render_passes;
use re::system::{Pass, System, RenderableObject};
use re::utils::bufferize_data;
use re::window::Window;
use re::pipeline_cache::PipelineSpec;
use re::mesh::{PrimitiveTopology, VertexType};

use vulkano::command_buffer::DynamicState;
use vulkano::pipeline::viewport::Viewport;

use std::collections::HashMap;

use nalgebra_glm::*;

use tests_render_engine::{FlyCamera, relative_path};
use tests_render_engine::mesh::{load_obj, fullscreen_quad, PosTexNormTan};

/*
const SHADOW_MAP_DIMS: [u32; 2] = [6144, 1024];
const PATCH_DIMS: [f32; 2] = [1024.0, 1024.0];
*/
const SHADOW_MAP_DIMS: [u32; 2] = [12_288, 2048];
const PATCH_DIMS: [f32; 2] = [2048.0, 2048.0];

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
        // custom images, we use none
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
        vtype: VertexType::<PosTexNormTan>::new(),
    };
    let pipe_caster = pipe_spec_caster.concrete(device.clone(), rpass_shadow.clone());

    let mut all_objects = HashMap::new();

    // used in main loop
    let pipeline = objects[0]
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());

    all_objects.insert("cubemap_view", vec![quad]);
    all_objects.insert("shadow", objects.clone().iter().flat_map(|obj| {
        let objects = convert_to_shadow_casters(queue.clone(), pipe_caster.clone(), obj.clone());

        objects.iter().map(|obj| {
            RenderableObject {
                pipeline_spec: pipe_spec_caster.clone(),
                ..obj.clone()
            }
        }).collect::<Vec<RenderableObject>>()
    }).collect());


    while !window.update() {
        // update camera and light
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let light_buffer = light.get_buffer(queue.clone());
        let camera_light_set = pds_for_buffers(pipeline.clone(), &[camera_buffer, light_buffer], 3).unwrap(); // 0 is the descriptor set idx

        all_objects.insert("geometry", objects.clone().iter_mut().map(|obj| {
            // add camera set to each object before adding it to the scene
            obj.custom_sets.push(camera_light_set.clone());
            obj.clone()
        }).collect());

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
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

    fn get_buffer(&self, queue: Queue) -> Buffer {
        // let time = get_elapsed(self.start_time) / 4.0;
        let data = Light {
            // position: [time.sin(), 10.0, time.cos(), 0.0],
            position: [0.0, 10.0, 0.0, 0.0],
            power: 1.0,
        };

        bufferize_data(queue.clone(), data)
    }
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

    view_directions
        .iter()
        .zip(&up_directions)
        .zip(&patch_positions)
        .map(|((dir, up), patch_pos): ((&Vec3, &Vec3), &[f32; 2])| {
            let light_pos = vec3(0.0, 10.0, 0.0);
            let view_matrix: [[f32; 4]; 4] = look_at(
                &light_pos,
                &(light_pos + dir),
                up,
            )
            .into();
            let view_buffer = bufferize_data(queue.clone(), view_matrix);
            let set = pds_for_buffers(pipeline.clone(), &[view_buffer], 2).unwrap();

            // all sets for the dragon we're currently creating
            // we take the model set from the base dragon
            // (set 0)
            let custom_sets = vec![
                model_set.clone(),
                proj_set.clone(),
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
