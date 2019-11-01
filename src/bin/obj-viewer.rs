use render_engine::collection_cache::pds_for_buffers;
use render_engine::input::get_elapsed;
use render_engine::mesh::{PrimitiveTopology, VertexType, ObjectPrototype};
use render_engine::pipeline_cache::PipelineSpec;
use render_engine::render_passes;
use render_engine::system::{Pass, System, RenderableObject};
use render_engine::utils::bufferize_data;
use render_engine::window::Window;
use render_engine::{Buffer, Queue};

use std::collections::HashMap;
use std::env;
use std::path::Path;

use nalgebra_glm::{Mat4, vec3, scale};

use tests_render_engine::mesh::{
    add_tangents_multi, convert_meshes, load_obj, load_textures, VPosTexNormTan,
};
use tests_render_engine::{relative_path, FlyCamera};

fn main() {
    // get path to load_obj
    let args: Vec<String> = env::args().collect();
    let path = if args.len() < 2 {
        println!("No path given to load!");
        return;
    } else {
        Path::new(&args[1])
    };

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
        // custom images, we use none
        HashMap::new(),
        "resolve_color",
    );

    window.set_render_pass(render_pass.clone());

    // initialize camera
    let mut camera = FlyCamera::default();

    // light
    let light = MovingLight::new();

    // create a pipeline identical to the one that will be used to draw all
    // objects. needed to create sets for the textures
    let pipeline_spec = PipelineSpec {
        vs_path: relative_path("shaders/obj-viewer/vert.glsl"),
        fs_path: relative_path("shaders/obj-viewer/frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        vtype: VertexType::<VPosTexNormTan>::new(),
    };
    let pipeline = pipeline_spec.concrete(device.clone(), render_pass.clone());

    // load meshes and materials
    let (models, materials) = load_obj(&path).expect("Couldn't open OBJ file");
    let meshes = add_tangents_multi(&convert_meshes(&models));
    let textures_path = path.parent().expect("Given path has no parent!");
    println!("Searching for textures in {:?}", textures_path);
    let texture_sets = load_textures(queue.clone(), pipeline.clone(), textures_path, &materials, 1);

    // create a default Material (todo: get rid of this)
    let material = Material {
        ambient: [1.0, 1.0, 1.0, 0.0],
        diffuse: [1.0, 1.0, 1.0, 0.0],
        specular: [1.0, 1.0, 1.0, 0.0],
        shininess: [32.0, 0.0, 0.0, 0.0],
        use_texture: [1.0, 0.0, 0.0, 0.0],
    };
    let material_buffer = bufferize_data(queue.clone(), material);
    let model: [[f32; 4]; 4] = scale(&Mat4::identity(), &vec3(0.1, 0.1, 0.1)).into();
    let model_buffer = bufferize_data(queue.clone(), model);
    let material_model_set = pds_for_buffers(pipeline.clone(), &[material_buffer, model_buffer], 0).unwrap();

    // combine the meshes and textures to create a list of renderable objects
    let objects: Vec<RenderableObject> = meshes
        .into_iter()
        .enumerate()
        .map(|(idx, mesh)| {
            let model = &models[idx];

            let mat_idx = if let Some(idx) = model.mesh.material_id {
                idx
            } else {
                println!("Model {} has no material id! Using 0.", model.name);
                0
            };
            let texture_set = texture_sets[mat_idx].clone();

            ObjectPrototype {
                vs_path: relative_path("shaders/obj-viewer/vert.glsl"),
                fs_path: relative_path("shaders/obj-viewer/frag.glsl"),
                fill_type: PrimitiveTopology::TriangleList,
                read_depth: true,
                write_depth: true,
                mesh,
                custom_sets: vec![material_model_set.clone(), texture_set],
                custom_dynamic_state: None,
            }
            .into_renderable_object(queue.clone())
        })
        .collect();

    println!("Objects Loaded: {}", objects.len());
    let mut all_objects = HashMap::new();

    // used in main loop
    while !window.update() {
        // update camera and light
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        let light_buffer = light.get_buffer(queue.clone());
        let camera_light_set =
            pds_for_buffers(pipeline.clone(), &[camera_buffer, light_buffer], 2).unwrap(); // 0 is the descriptor set idx

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

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

#[allow(dead_code)]
struct Light {
    direction: [f32; 4],
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
        let time = get_elapsed(self.start_time) / 4.0;
        let data = Light {
            direction: [time.sin(), 2.0, time.cos(), 0.0],
            power: 1.0,
        };

        bufferize_data(queue.clone(), data)
    }
}

#[allow(dead_code)]
struct Material {
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
    shininess: [f32; 4],
    use_texture: [f32; 4],
}
