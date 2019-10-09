use render_engine as re;

use re::producer::ProducerCollection;
use re::world::ObjectSpecBuilder;
use re::{mesh_gen, App, OrbitCamera};

use vulkano::device::Device;
use vulkano::buffer::BufferAccess;

use nalgebra_glm as glm;

use std::path::PathBuf;
use std::sync::Arc;

const LIGHT_POS: [f32; 3] = [10.0, 2.0, 5.0];

fn main() {
    // paths and loading meshes
    let mesh_path = relative_path("meshes/dragon.obj");
    let mesh = mesh_gen::load_obj(&mesh_path).unwrap();

    let vs_path = relative_path("shaders/lighting_object_vert.glsl");
    let object_fs_path = relative_path("shaders/lighting_object_frag.glsl");
    let light_fs_path = relative_path("shaders/lighting_light_frag.glsl");

    // app init
    let mut app = App::new();
    let mut world_com = app.get_world_com();

    // creating buffers
    let object_matrix: Matrix = glm::Mat4::identity().into();
    let light_matrix: Matrix = glm::scale(
        &glm::translate(&glm::Mat4::identity(), &glm::vec3(LIGHT_POS[0], LIGHT_POS[1], LIGHT_POS[2])),
        &glm::vec3(0.2, 0.2, 0.2),
    )
    .into();
    let object_matrix_buffer = buffer_for_matrix(app.get_device(), object_matrix);
    let light_matrix_buffer = buffer_for_matrix(app.get_device(), light_matrix);
    let light_info_buffer = create_light_info(app.get_device());

    // adding objects
    let dragon_spec = ObjectSpecBuilder::default()
        .mesh(mesh)
        .shaders(vs_path.clone(), object_fs_path.clone())
        .additional_resources(vec![object_matrix_buffer, light_info_buffer])
        .build();
    let light_spec = ObjectSpecBuilder::default()
        .shaders(vs_path.clone(), light_fs_path.clone())
        .additional_resources(vec![light_matrix_buffer])
        .build();

    world_com.add_object_from_spec("dragon", dragon_spec);
    world_com.add_object_from_spec("light", light_spec);

    // change camera to one with a farther orbit distance
    let mut camera = OrbitCamera::default();
    camera.orbit_distance = 24.0;
    let producers = ProducerCollection::new(vec![], vec![Box::new(camera)]);
    app.set_producers(producers);

    // main loop
    while !app.done {
        app.draw_frame();
    }

    app.print_fps();
}

fn buffer_for_matrix(device: Arc<Device>, data: Matrix) -> Arc<dyn BufferAccess + Send + Sync> {
    let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<Matrix>::new(
        device.clone(),
        vulkano::buffer::BufferUsage::all(),
    );

    Arc::new(pool.next(data).unwrap())
}

fn create_light_info(device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
    let data = LightInfo {
        light_power: 5.0,
        light_pos: LIGHT_POS,
        light_color: [1.0, 1.0, 1.0],
        object_color: [1.0, 0.5, 0.31],
    };

    let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<LightInfo>::new(
        device.clone(),
        vulkano::buffer::BufferUsage::all(),
    );

    Arc::new(pool.next(data).unwrap())
}

fn relative_path(local_path: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), local_path].iter().collect()
}

struct LightInfo {
    light_power: f32,
    light_pos: [f32; 3],
    light_color: [f32; 3],
    object_color: [f32; 3],
}

type Matrix = [[f32; 4]; 4];
