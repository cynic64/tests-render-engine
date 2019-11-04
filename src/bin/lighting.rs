use render_engine::input::get_elapsed;
use render_engine::mesh::{ObjectPrototype, PrimitiveTopology};
use render_engine::render_passes;
use render_engine::system::{Pass, System};
use render_engine::utils::{bufferize_data, load_texture, default_sampler};
use render_engine::window::Window;
use render_engine::collection::Data;

// TODO: reeeeee i shouldn't have to do this
use nalgebra_glm::*;
use vulkano::format::Format;

use std::collections::HashMap;
use std::sync::Arc;

use tests_render_engine::mesh::{add_tangents, convert_meshes, load_obj};
use tests_render_engine::{relative_path, OrbitCamera, Matrix4};

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
                "multisampeld_color",
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

    // create buffers for model matrix, light and materials
    let model_data: Matrix4 = translate(&Mat4::identity(), &vec3(0.0, -6.0, 0.0)).into();

    let mut light = Light {
        position: [10.0, 0.0, 0.0, 0.0],
        ambient: [0.3, 0.3, 0.3, 0.0],
        diffuse: [1.3, 1.3, 1.3, 0.0],
        specular: [1.5, 1.5, 1.5, 0.0],
    };

    // TODO: implement Copy for queue?
    let material_data = Material { shininess: 76.8 };

    // load texture
    let start_time = std::time::Instant::now();
    let diffuse_texture = load_texture(
        queue.clone(),
        &relative_path("textures/raptor-diffuse.png"),
        Format::R8G8B8A8Srgb,
    );
    let specular_texture = load_texture(
        queue.clone(),
        &relative_path("textures/raptor-specular.png"),
        Format::R8G8B8A8Unorm,
    );
    let normal_texture = load_texture(
        queue.clone(),
        &relative_path("textures/raptor-normal.png"),
        Format::R8G8B8A8Unorm,
    );
    println!("Time taken to load textures: {}s", get_elapsed(start_time));

    // initialize camera
    let mut camera = OrbitCamera::default();
    let camera_data = camera.get_data();

    // load mesh and create object
    let (mut models, _materials) =
        load_obj(&relative_path("meshes/raptor.obj")).expect("couldn't load OBJ");
    let basic_mesh = convert_meshes(&[models.remove(0)]).remove(0);
    let mesh = add_tangents(&basic_mesh);

    // 00 model 01 material 10 camera 20 light 30 diff 31 spec 32 norm
    let mut collection = (
        (model_data, material_data),
        (camera_data,),
        (light.clone(),),
        (diffuse_texture, specular_texture, normal_texture),
    );

    let mut object = ObjectPrototype {
        vs_path: relative_path("shaders/lighting/object_vert.glsl"),
        fs_path: relative_path("shaders/lighting/object_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        mesh,
        collection: collection.clone(),
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    // used in main loop
    let start_time = std::time::Instant::now();

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_data = camera.get_data();

        // update light
        let time = get_elapsed(start_time);
        let light_x = (time / 4.0).sin() * 20.0;
        let light_z = (time / 4.0).cos() * 20.0;
        light.position = [light_x, 0.0, light_z, 0.0];

        collection.1 = (camera_data,);
        collection.2 = (light.clone(),);

        object.collection = Arc::new(collection.clone());

        let mut all_objects = HashMap::new();
        all_objects.insert("geometry", vec![object.clone()]);

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
}

#[allow(dead_code)]
#[derive(Clone)]
struct Light {
    position: [f32; 4],
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
}

#[allow(dead_code)]
#[derive(Clone)]
struct Material {
    shininess: f32,
}

impl Data for Light {}
impl Data for Material {}
