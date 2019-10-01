use render_engine as re;

use re::camera::OrbitCamera;
use re::mesh_gen;
use re::producer::ProducerCollection;
use re::world::ObjectSpecBuilder;
use re::App;

use rand::Rng;

pub fn main() {
    let mut app = App::new();

    // create vertices and object specs
    let mut world_com = app.get_world_com();
    let mut rng = rand::thread_rng();
    for i in 0..10_000 {
        let x = 1.0 / (rng.gen::<f32>() * 2.0 - 1.0);
        let y = 1.0 / (rng.gen::<f32>() * 2.0 - 1.0);
        let z = 1.0 / (rng.gen::<f32>() * 2.0 - 1.0);
        let mesh = mesh_gen::create_vertices_for_cube([x, y, z], 0.2);
        let spec = ObjectSpecBuilder::default()
            .mesh(mesh)
            .build(app.get_device());

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
