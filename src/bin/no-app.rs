use render_engine as re;

/*
Annoyances:
ObjectSpecBuilder is great, but can only be used with world right now. Fix that.
Why do I have to manage queue and device? :(
*/

use re::mesh_gen;
use re::pipeline_cache::PipelineSpec;
use re::shaders::relative_path;
use re::system::RenderableObject;
use re::template_systems;
use re::utils::ibuf_from_vec;
use re::window::Window;

use vulkano::pipeline::input_assembly::PrimitiveTopology;

use std::collections::HashMap;

fn main() {
    let mut window = Window::new();
    let queue = window.get_queue();
    let device = queue.device().clone();

    let (mut system, mut producers) = template_systems::forward_with_depth(queue.clone());
    // TODO: which render pass does this refer to?
    let render_pass = system.get_passes()[0].get_render_pass().clone();
    window.set_render_pass(render_pass);

    let object_mesh = mesh_gen::create_vertices_for_cube([0.0, 0.0, 0.0], 1.0);
    let vbuf = object_mesh.vertices.create_vbuf(device.clone());
    let ibuf = ibuf_from_vec(device.clone(), &object_mesh.indices);
    let pipeline_spec = PipelineSpec {
        vs_path: relative_path("shaders/forward/default_vert.glsl"),
        fs_path: relative_path("shaders/forward/default_frag.glsl"),
        fill_type: PrimitiveTopology::TriangleList,
    };
    let object = RenderableObject {
        pipeline_spec,
        vbuf,
        ibuf,
    };
    let mut all_objects = HashMap::new();
    all_objects.insert("geometry", vec![object]);

    while !window.update() {
        // draw
        let swapchain_image = window.next_image();
        let swapchain_fut = window.get_future();
        let shared_resources = producers.get_shared_resources(device.clone());

        // draw_frame returns a future representing the completion of rendering
        let frame_fut = system.draw_frame(
            swapchain_image.dimensions(),
            all_objects.clone(),
            shared_resources,
            swapchain_image,
            swapchain_fut,
        );

        window.present_future(frame_fut);

        producers.update(window.get_frame_info());
    }
}
