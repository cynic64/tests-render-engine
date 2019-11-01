use render_engine as re;

/*
Annoyances:
Why do I have to manage queue and device? :(
*/

use re::collection_cache::pds_for_buffers;
use re::input::get_elapsed;
use re::mesh::{Mesh, ObjectPrototype, PrimitiveTopology};
use re::render_passes;
use re::system::{Pass, System};
use re::utils::{bufferize_data, load_texture};
use re::window::Window;

use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::format::Format;

use nalgebra_glm::*;

use std::collections::HashMap;
use std::sync::Arc;

use tests_render_engine::mesh::{
    add_tangents, convert_meshes, load_obj, merge, only_pos_from_ptnt, wireframe, VPosTexNormTan,
};
use tests_render_engine::{default_sampler, relative_path, OrbitCamera};

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

    // create buffer for model matrix
    let model_data: [[f32; 4]; 4] = translate(
        &scale(&Mat4::identity(), &vec3(1.0, 1.0, 1.0)),
        &vec3(0.0, -6.0, 0.0),
    )
    .into();
    let model_buffer = bufferize_data(queue.clone(), model_data);

    // initialize camera
    let mut camera = OrbitCamera::default();

    // load meshes and create objects
    let (mut models, _materials) =
        load_obj(&relative_path("meshes/raptor.obj")).expect("Couldn't load OBJ file");
    let basic_mesh = convert_meshes(&[models.remove(0)]).remove(0);
    let raptor_mesh = add_tangents(&basic_mesh);
    let normals_mesh = normals_vis(&raptor_mesh);

    let mut raptor = ObjectPrototype {
        vs_path: relative_path("shaders/normal-mapping/object_vert.glsl"),
        fs_path: relative_path("shaders/normal-mapping/object_frag.glsl"),
        read_depth: true,
        write_depth: true,
        fill_type: PrimitiveTopology::TriangleList,
        mesh: raptor_mesh,
        custom_sets: vec![],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    let mut normals = ObjectPrototype {
        vs_path: relative_path("shaders/normal-mapping/debug_vert.glsl"),
        fs_path: relative_path("shaders/normal-mapping/debug_frag.glsl"),
        read_depth: true,
        write_depth: true,
        fill_type: PrimitiveTopology::LineList,
        mesh: normals_mesh,
        custom_sets: vec![],
        custom_dynamic_state: None,
    }
    .into_renderable_object(queue.clone());

    // textures
    let normal_texture = load_texture(
        queue.clone(),
        &relative_path("textures/raptor-normal.png"),
        Format::R8G8B8A8Unorm,
    );

    // light
    let mut light = Light {
        position: [10.0, 0.0, 0.0],
    };

    // used in main loop
    let mut all_objects = HashMap::new();
    let raptor_pipe = raptor
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let normals_pipe = normals
        .pipeline_spec
        .concrete(device.clone(), render_pass.clone());
    let sampler = default_sampler(device.clone());
    let start_time = std::time::Instant::now();

    while !window.update() {
        // update camera and camera buffer
        camera.update(window.get_frame_info());
        let camera_buffer = camera.get_buffer(queue.clone());

        // update light
        let time = get_elapsed(start_time);
        let light_x = (time / 4.0).sin() * 20.0;
        let light_z = (time / 4.0).cos() * 20.0;
        light.position = [light_x, 0.0, light_z];
        let light_buffer = bufferize_data(queue.clone(), light.clone());

        // create set
        raptor.custom_sets = vec![Arc::new(
            PersistentDescriptorSet::start(raptor_pipe.clone(), 0)
                .add_buffer(model_buffer.clone())
                .unwrap()
                .add_buffer(camera_buffer.clone())
                .unwrap()
                .add_buffer(light_buffer.clone())
                .unwrap()
                .add_sampled_image(normal_texture.clone(), sampler.clone())
                .unwrap()
                .build()
                .unwrap(),
        )];

        let normals_set = pds_for_buffers(
            normals_pipe.clone(),
            &[model_buffer.clone(), camera_buffer.clone()],
            0,
        )
        .unwrap(); // 0 is the descriptor set idx
        normals.custom_sets = vec![normals_set];

        if window.get_frame_info().keys_down.c {
            raptor.pipeline_spec.fs_path =
                relative_path("shaders/normal-mapping/object_frag_debug.glsl");
        } else {
            raptor.pipeline_spec.fs_path = relative_path("shaders/normal-mapping/object_frag.glsl");
        }

        let objects = if window.get_frame_info().keys_down.c {
            vec![raptor.clone(), normals.clone()]
        } else {
            vec![raptor.clone()]
        };
        all_objects.insert("geometry", objects);

        // draw
        system.render_to_window(&mut window, all_objects.clone());
    }

    println!("FPS: {}", window.get_fps());
    system.print_stats();
}

fn normals_vis(mesh: &Mesh<VPosTexNormTan>) -> Mesh<VPosColor> {
    // produces a mesh of type VPos, we need VPosColor
    let wireframe_pos_only = wireframe(&only_pos_from_ptnt(&mesh));
    let wireframe_verts: Vec<VPosColor> = wireframe_pos_only
        .vertices
        .iter()
        .map(|vertex| VPosColor {
            position: vertex.position,
            color: [0.0, 0.0, 0.0],
        })
        .collect();
    let wireframe_mesh = Mesh {
        vertices: wireframe_verts,
        indices: wireframe_pos_only.indices,
    };

    let vertices: Vec<VPosColor> = mesh
        .vertices
        .iter()
        .flat_map(|v| {
            let normal = make_vec3(&v.normal);
            let tangent = make_vec3(&v.tangent);
            let bitangent = tangent.cross(&normal);
            let position = make_vec3(&v.position);

            vec![
                // line to show normal, colored red
                VPosColor {
                    position: v.position,
                    color: [1.0, 0.0, 0.0],
                },
                VPosColor {
                    position: (position + normal * 0.2).into(),
                    color: [1.0, 0.0, 0.0],
                },
                // line to show normal, colored red
                VPosColor {
                    position: v.position,
                    color: [0.0, 1.0, 0.0],
                },
                VPosColor {
                    position: (position + tangent * 0.2).into(),
                    color: [0.0, 1.0, 0.0],
                },
                // line to show bitangent, colored blue
                VPosColor {
                    position: v.position,
                    color: [0.0, 0.0, 1.0],
                },
                VPosColor {
                    position: (position + bitangent * 0.2).into(),
                    color: [0.0, 0.0, 1.0],
                },
            ]
        })
        .collect();

    let indices: Vec<u32> = (0..vertices.len()).map(|x| x as u32).collect();
    let normals_mesh = Mesh { vertices, indices };

    merge(&[wireframe_mesh, normals_mesh])
}

#[derive(Clone)]
struct Light {
    position: [f32; 3],
}

#[derive(Default, Debug, Clone, Copy)]
struct VPosColor {
    position: [f32; 3],
    color: [f32; 3],
}
vulkano::impl_vertex!(VPosColor, position, color);
