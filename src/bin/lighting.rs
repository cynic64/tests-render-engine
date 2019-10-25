use render_engine as re;

/*
Annoyances:
Why do I have to manage queue and device? :(
*/

use re::input::get_elapsed;
use re::render_passes;
use re::system::{Pass, System};
use re::utils::{bufferize_data, load_texture};
use re::mesh::{ObjectPrototype, PrimitiveTopology};
use re::window::Window;

// TODO: reeeeee i shouldn't have to do this
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::format::Format;
use nalgebra_glm::*;

use std::collections::HashMap;
use std::sync::Arc;

use tests_render_engine::{default_sampler, relative_path, OrbitCamera};
use tests_render_engine::mesh::{add_tangents, load_obj_single};

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
    let model_data: [[f32; 4]; 4] = translate(&Mat4::identity(), &vec3(0.0, -6.0, 0.0)).into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    let mut light = Light {
        position: [10.0, 0.0, 0.0, 0.0],
        ambient: [0.3, 0.3, 0.3, 0.0],
        diffuse: [1.3, 1.3, 1.3, 0.0],
        specular: [1.5, 1.5, 1.5, 0.0],
    };

    // TODO: implement Copy for queue?
    let material_buffer = bufferize_data(
        queue.clone(),
        Material {
            shininess: 76.8,
        },
    );

    // load texture
    let start_time = std::time::Instant::now();
    let diffuse_texture = load_texture(queue.clone(), &relative_path("textures/raptor-diffuse.png"), Format::R8G8B8A8Srgb);
    let specular_texture = load_texture(queue.clone(), &relative_path("textures/raptor-specular.png"), Format::R8G8B8A8Unorm);
    let normal_texture = load_texture(queue.clone(), &relative_path("textures/raptor-normal.png"), Format::R8G8B8A8Unorm);
    println!("Time taken to load textures: {}s", get_elapsed(start_time));

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load mesh and create object
    let basic_mesh = load_obj_single(&relative_path("meshes/raptor.obj"));
    let mesh = add_tangents(&basic_mesh);

    let mut object = ObjectPrototype {
        vs_path: relative_path("shaders/lighting/object_vert.glsl"),
        fs_path: relative_path("shaders/lighting/object_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
        read_depth: true,
        write_depth: true,
        mesh,
        custom_sets: vec![],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    // used in main loop
    let pipeline = object.pipeline_spec.concrete(device.clone(), render_pass.clone());
    let start_time = std::time::Instant::now();
    let sampler = default_sampler(device.clone());

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        // update light
        let time = get_elapsed(start_time);
        let light_x = (time / 4.0).sin() * 20.0;
        let light_z = (time / 4.0).cos() * 20.0;
        light.position = [light_x, 0.0, light_z, 0.0];
        let light_buffer = bufferize_data(queue.clone(), light.clone());

        // TODO: use multiple custom sets instead of this solution
        object.custom_sets = vec![Arc::new(
            PersistentDescriptorSet::start(pipeline.clone(), 0)
                .add_buffer(model_buffer.clone())
                .unwrap()
                .add_buffer(camera_buffer)
                .unwrap()
                .add_buffer(light_buffer)
                .unwrap()
                .add_buffer(material_buffer.clone())
                .unwrap()
                .add_sampled_image(diffuse_texture.clone(), sampler.clone())
                .unwrap()
                .add_sampled_image(specular_texture.clone(), sampler.clone())
                .unwrap()
                .add_sampled_image(normal_texture.clone(), sampler.clone())
                .unwrap()
                .build()
                .unwrap(),
        )];

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
struct Material {
    shininess: f32,
}
