use render_engine as re;

use re::world::ObjectSpecBuilder;
use re::App;

pub fn main() {
    let mut app = App::new();
    let mut world_com = app.get_world_com();
    let spec = ObjectSpecBuilder::default().build();
    world_com.add_object_from_spec("cube", spec);

    while !app.done {
        app.draw_frame();
    }

    app.print_fps();
}
