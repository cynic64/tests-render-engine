use render_engine as re;

use re::{App, mesh_gen, OrbitCamera};
use re::world::ObjectSpecBuilder;
use re::producer::ProducerCollection;

use std::path::PathBuf;

pub fn main() {
    let mut app = App::new();
    let mut world_com = app.get_world_com();

    let path = relative_path("dragon.obj");
    let dragon_mesh = mesh_gen::load_obj(&path).unwrap();
    let dragon = ObjectSpecBuilder::default()
        .mesh(dragon_mesh)
        .build(app.get_device());
    world_com.add_object_from_spec("dragon", dragon);

    // change camera to one with a farther orbit distance
    let mut camera = OrbitCamera::default();
    camera.orbit_distance = 16.0;
    let producers = ProducerCollection::new(vec![Box::new(camera)]);
    app.set_producers(producers);

    while !app.done {
        app.draw_frame();
    }

    app.print_fps();
}

fn relative_path(local_path: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), local_path].iter().collect()
}
