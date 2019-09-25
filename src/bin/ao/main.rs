// TODO: you shouldn't have to use vulkano much
use render_engine::*;
use render_engine::input::VirtualKeyCode;

use vulkano::pipeline::GraphicsPipeline;
use vulkano::framebuffer::Subpass;
use vulkano::buffer::BufferAccess;
use vulkano::device::Device;
use vulkano::format::Format;

use std::path::PathBuf;
use std::sync::Arc;

use nalgebra_glm::*;

fn main() {
    let mut app = App::new();

    let geo_render_pass = Arc::new(
        vulkano::single_pass_renderpass!(
            app.get_device(),
            attachments: {
                position: {
                    load: Clear,
                    store: Store,
                    format: Format::R32G32B32A32Sfloat,
                    samples: 1,
                },
                color: {
                    load: Clear,
                    store: Store,
                    format: Format::R8G8B8A8Unorm,
                    samples: 1,
                },
                normal: {
                    load: Clear,
                    store: Store,
                    format: Format::R16G16B16A16Sfloat,
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: Store,
                    format: Format::D16Unorm,
                    samples: 1,
                }
            },
            pass: {
                color: [position, color, normal],
                depth_stencil: {depth}
            }
        )
        .unwrap(),
    );

    // load shaders for each pass
    let geo_vs_path = relative_path("shaders/ao_geo_vert.glsl");
    let geo_fs_path = relative_path("shaders/ao_geo_frag.glsl");
    let geo_shaders =
        shaders::ShaderSystem::load_from_file(app.get_device(), &geo_vs_path, &geo_fs_path);

    let deferred_vs_path = relative_path("shaders/ao_ao_vert.glsl");
    let ao_fs_path = relative_path("shaders/ao_ao_frag.glsl");
    let ao_shaders =
        shaders::ShaderSystem::load_from_file(app.get_device(), &deferred_vs_path, &ao_fs_path);;

    // create system
    let pass1 = system::ComplexPass {
        images_needed: vec![],
        images_created: vec!["position", "color", "normal", "depth"],
        resources_needed: vec!["view_proj"],
        render_pass: geo_render_pass,
    };

    let ao_render_pass = render_passes::basic(app.get_device());
    let (ao_vs_main, ao_fs_main) = ao_shaders.get_entry_points();
    let ao_pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<system::SimpleVertex>()
            .vertex_shader(ao_vs_main, ())
            .primitive_topology(PrimitiveTopology::TriangleStrip)
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(ao_fs_main, ())
            .render_pass(Subpass::from(ao_render_pass.clone(), 0).unwrap())
            .build(app.get_device())
            .unwrap(),
    );
    let pass2 = system::SimplePass {
        images_created: vec!["ao_color"],
        images_needed: vec!["position", "normal"],
        resources_needed: vec!["view_proj", "ao_samples"],
        render_pass: ao_render_pass,
        pipeline: ao_pipeline,
    };

    let output_tag = "ao_color";
    let passes: Vec<Box<dyn system::Pass>> = vec![Box::new(pass1), Box::new(pass2)];
    let system = system::System::new(app.get_queue(), passes, output_tag);
    app.set_system(system);

    // create producers for AO noise and custom camera
    let mut camera = OrbitCamera::default();
    camera.orbit_distance = 20.0;
    let camera_p = Box::new(camera);
    let noise_p = Box::new(AONoiseProducer::new(app.get_device()));
    let producer_collection = producer::ProducerCollection::new(vec![camera_p, noise_p]);
    app.set_producers(producer_collection);


    let mut world_com = app.get_world_com();

    let path = relative_path("meshes/happy.obj");
    let dragon_mesh = mesh_gen::load_obj(&path).unwrap();
    let dragon = ObjectSpecBuilder::default()
        .mesh(dragon_mesh)
        .shaders(geo_shaders)
        .build(app.get_device());
    world_com.add_object_from_spec("dragon", dragon);

    while !app.done {
        let frame_info = app.get_frame_info();
        if frame_info.keydowns.contains(&VirtualKeyCode::Escape) {
            break;
        }

        app.draw_frame();
    }

    app.print_fps();
}

fn relative_path(local_path: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), local_path].iter().collect()
}

struct AONoiseProducer {
    noise_buffer: Arc<dyn BufferAccess + Send + Sync>,
}

#[allow(dead_code)]
struct AOSamples {
    samples: [[f32; 4]; 32],
}

impl AONoiseProducer {
    fn new(device: Arc<Device>) -> Self {
        let mut ssao_kernel = [[0.0; 4]; 32];
        for x in 0..32 {
            let mut sample = vec3(
                rand::random::<f32>() * 2.0 - 1.0,
                rand::random::<f32>() * 2.0 - 1.0,
                rand::random::<f32>(),
            );
            sample = normalize(&sample);
            sample *= rand::random::<f32>();
            let mut scale = (x as f32) / 32.0;
            scale = lerp_scalar(0.1, 1.0, scale * scale);
            sample *= scale;
            ssao_kernel[x] = [sample.x, sample.y, sample.z, 0.0];
        }

        let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<AOSamples>::new(
            device.clone(),
            vulkano::buffer::BufferUsage::all(),
        );

        let noise_buffer = {
            let uniform_data = AOSamples {
                samples: ssao_kernel.clone(),
            };

            pool.next(uniform_data).unwrap()
        };

        Self {
            noise_buffer: Arc::new(noise_buffer),
        }
    }
}

use producer::ResourceProducer;
impl ResourceProducer for AONoiseProducer {
    fn create_buffer(&self, _device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
        self.noise_buffer.clone()
    }

    fn name(&self) -> &str {
        "ao_samples"
    }
}
