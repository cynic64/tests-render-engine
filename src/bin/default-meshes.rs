use render_engine as re;

use re::camera::OrbitCamera;
use re::mesh_gen;
use re::producer::ProducerCollection;
use re::world::{ObjectSpecBuilder, PrimitiveTopology};
use re::App;

pub fn main() {
    let mut app = App::new();

    // create vertices and object specs
    let verts_cube_1 = mesh_gen::create_vertices_for_cube([0.0, 0.0, 0.0], 1.0);
    let verts_cube_2 = mesh_gen::create_vertices_for_cube([2.0, 1.0, 3.0], 0.5);
    let verts_cube_edges = mesh_gen::create_vertices_for_cube_edges([-2.0, 1.0, 1.0], 0.6);

    let spec_cube_1 = ObjectSpecBuilder::default()
        .mesh(verts_cube_1)
        .build(app.get_device());
    let spec_cube_2 = ObjectSpecBuilder::default()
        .mesh(verts_cube_2)
        .build(app.get_device());
    let spec_cube_edges = ObjectSpecBuilder::default()
        .mesh(verts_cube_edges)
        .fill_type(PrimitiveTopology::LineList)
        .build(app.get_device());

    let mut world_com = app.get_world_com();
    world_com.add_object_from_spec("cube 1", spec_cube_1);
    world_com.add_object_from_spec("cube 2", spec_cube_2);
    world_com.add_object_from_spec("cube edges", spec_cube_edges);

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
