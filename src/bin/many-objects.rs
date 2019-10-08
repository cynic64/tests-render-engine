use render_engine as re;

use re::camera::OrbitCamera;
use re::mesh_gen;
use re::producer::ProducerCollection;
use re::world::ObjectSpecBuilder;
use re::App;


use vulkano::device::Device;
use vulkano::buffer::BufferAccess;

use nalgebra_glm as glm;

use rand::Rng;

use std::sync::Arc;
use std::path::PathBuf;

pub fn main() {
    let mut app = App::new();

    // create vertices and object specs
    let mut world_com = app.get_world_com();

    let (vs_path, fs_path) = (relative_path("shaders/many_objects_vert.glsl"), relative_path("shaders/many_objects_frag.glsl"));

    for i in 0..1_000 {
        let mesh = mesh_gen::create_vertices_for_cube([0.0, 0.0, 0.0], 0.2);
        let res = random_buffer(app.get_device());

        let spec = ObjectSpecBuilder::default()
            .mesh(mesh)
            .additional_resources(res)
            .shaders(vs_path.clone(), fs_path.clone())
            .build();

        world_com.add_object_from_spec(&i.to_string(), spec);
    }

    // change camera to one with a farther orbit distance
    let mut camera = OrbitCamera::default();
    camera.orbit_distance = 10.0;
    let producers = ProducerCollection::new(vec![], vec![Box::new(camera)]);
    app.set_producers(producers);

    while !app.done {
        app.draw_frame();
    }

    app.print_fps();
}

pub fn random_buffer(device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
    let mut rng = rand::thread_rng();

    let x = 1.0 / (rng.gen::<f32>() * 2.0 - 1.0);
    let y = 1.0 / (rng.gen::<f32>() * 2.0 - 1.0);
    let z = 1.0 / (rng.gen::<f32>() * 2.0 - 1.0);

    let data: ModelMatrix = glm::translate(&glm::Mat4::identity(), &glm::vec3(x, y, z)).into();

    let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<ModelMatrix>::new(
        device.clone(),
        vulkano::buffer::BufferUsage::all(),
    );

    Arc::new(pool.next(data).unwrap())
}

type ModelMatrix = [[f32; 4]; 4];

pub fn relative_path(local_path: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), local_path].iter().collect()
}
