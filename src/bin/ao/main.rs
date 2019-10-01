// TODO: you shouldn't have to use vulkano much
// and don't import everything from render-engine
use render_engine::input::{FrameInfo, VirtualKeyCode};
use render_engine::producer::{BufferProducer, ImageProducer};
use render_engine::*;

use vulkano::buffer::BufferAccess;
use vulkano::device::{Device, Queue};
use vulkano::format::Format;
use vulkano::framebuffer::Subpass;
use vulkano::image::traits::ImageViewAccess;
use vulkano::image::{Dimensions, ImmutableImage};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::sync::GpuFuture;

use std::path::PathBuf;
use std::sync::Arc;

use nalgebra_glm::*;

// TODO: perf regression tests for all of these

fn main() {
    let path = relative_path("meshes/dragon.obj");
    let happy_mesh = mesh_gen::load_obj(&path).unwrap();

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

    let deferred_vs_path = relative_path("shaders/ao_simple_vert.glsl");
    let ao_fs_path = relative_path("shaders/ao_ao_frag.glsl");
    let ao_shaders =
        shaders::ShaderSystem::load_from_file(app.get_device(), &deferred_vs_path, &ao_fs_path);;

    let blur_fs_path = relative_path("shaders/ao_blur_frag.glsl");
    let blur_shaders =
        shaders::ShaderSystem::load_from_file(app.get_device(), &deferred_vs_path, &blur_fs_path);

    // TODO: don't load deferred shader twice
    let final_fs_path = relative_path("shaders/ao_final_frag.glsl");
    let final_shaders =
        shaders::ShaderSystem::load_from_file(app.get_device(), &deferred_vs_path, &final_fs_path);

    // create system
    let geo_pass = system::Pass::Complex {
        name: "geometry",
        images_needed: vec![],
        images_created: vec!["position", "color", "normal", "depth"],
        buffers_needed: vec!["view_proj"],
        render_pass: geo_render_pass,
    };

    /*
    let ao_render_pass = Arc::new(
        vulkano::single_pass_renderpass!(
            app.get_device(),
            attachments: {
                color: {
                    load: DontCare,
                    store: Store,
                    format: Format::R16Sfloat,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap(),
    );
     */
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
    let ao_pass = system::Pass::Simple {
        name: "ao",
        images_created: vec!["ao_raw"],
        images_needed: vec!["position", "normal", "ao_noise"],
        buffers_needed: vec!["view_proj", "ao_samples", "dimensions"],
        render_pass: ao_render_pass,
        pipeline: ao_pipeline,
    };

    /*
    let blur_render_pass = Arc::new(
        vulkano::single_pass_renderpass!(
            app.get_device(),
            attachments: {
                color: {
                    load: DontCare,
                    store: Store,
                    format: Format::R16Sfloat,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap(),
    );

    let (blur_vs_main, blur_fs_main) = blur_shaders.get_entry_points();
    let blur_pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<system::SimpleVertex>()
            .vertex_shader(blur_vs_main, ())
            .primitive_topology(PrimitiveTopology::TriangleStrip)
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(blur_fs_main, ())
            .render_pass(Subpass::from(blur_render_pass.clone(), 0).unwrap())
            .build(app.get_device())
            .unwrap(),
    );
    let blur_pass = system::SimplePass {
        images_created: vec!["ao_blur"],
        images_needed: vec!["ao_raw"],
        resources_needed: vec![],
        render_pass: blur_render_pass,
        pipeline: blur_pipeline,
    };

    let final_render_pass = render_passes::basic(app.get_device());
    let (final_vs_main, final_fs_main) = final_shaders.get_entry_points();
    let final_pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<system::SimpleVertex>()
            .vertex_shader(final_vs_main, ())
            .primitive_topology(PrimitiveTopology::TriangleStrip)
            .viewports_dynamic_scissors_irrelevant(1)
            .fragment_shader(final_fs_main, ())
            .render_pass(Subpass::from(final_render_pass.clone(), 0).unwrap())
            .build(app.get_device())
            .unwrap(),
    );
    let final_pass = system::SimplePass {
        images_created: vec!["final_color"],
        images_needed: vec!["ao_blur", "color"],
        resources_needed: vec![],
        render_pass: final_render_pass,
        pipeline: final_pipeline,
    };
    */

    // let output_tag = "final_color";
    let output_tag = "ao_raw";
    let passes: Vec<system::Pass> = vec![geo_pass, ao_pass];
    // vec![Box::new(geo_pass), Box::new(ao_pass), Box::new(blur_pass), Box::new(final_pass)];
    let system = system::System::new(app.get_queue(), passes, output_tag);
    app.set_system(system);

    // create producers for AO noise and custom camera
    let mut camera = OrbitCamera::default();
    camera.orbit_distance = 20.0;
    let camera_p = Box::new(camera);
    let sample_p = Box::new(AOSampleProducer::new(app.get_device()));
    let noise_p = Box::new(AONoiseTexProducer::new(app.get_queue()));
    let dims_p = Box::new(DimensionsProducer::new());
    let producer_collection =
        producer::ProducerCollection::new(vec![noise_p], vec![camera_p, sample_p, dims_p]);
    app.set_producers(producer_collection);

    let mut world_com = app.get_world_com();

    let happy = ObjectSpecBuilder::default()
        .mesh(happy_mesh)
        .shaders(geo_shaders.clone())
        .build(app.get_device());
    world_com.add_object_from_spec("happy", happy);

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

struct AOSampleProducer {
    noise_buffer: Arc<dyn BufferAccess + Send + Sync>,
}

#[allow(dead_code)]
struct AOSamples {
    samples: [[f32; 4]; 64],
}

impl AOSampleProducer {
    fn new(device: Arc<Device>) -> Self {
        let mut ssao_kernel = [[0.0; 4]; 64];
        for x in 0..64 {
            let mut sample = vec3(
                rand::random::<f32>() * 2.0 - 1.0,
                rand::random::<f32>() * 2.0 - 1.0,
                rand::random::<f32>() * 2.0 - 1.0,
                // rand::random::<f32>(),
            );
            sample = normalize(&sample);
            sample *= rand::random::<f32>();
            let mut scale = (x as f32) / 64.0;
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

impl BufferProducer for AOSampleProducer {
    fn create_buffer(&self, _device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
        self.noise_buffer.clone()
    }

    fn name(&self) -> &str {
        "ao_samples"
    }
}

struct AONoiseTexProducer {
    noise_image: Arc<dyn ImageViewAccess + Send + Sync>,
}

impl AONoiseTexProducer {
    fn new(queue: Arc<Queue>) -> Self {
        let ssao_noise: Vec<[f32; 4]> = (0..16)
            .map(|_| {
                [
                    rand::random::<f32>() * 2.0 - 1.0,
                    rand::random::<f32>() * 2.0 - 1.0,
                    0.0,
                    0.0,
                ]
            })
            .collect();

        let (noise_tex, noise_tex_future) = ImmutableImage::from_iter(
            ssao_noise.iter().cloned(),
            Dimensions::Dim2d {
                width: 4,
                height: 4,
            },
            Format::R32G32B32A32Sfloat,
            queue.clone(),
        )
        .unwrap();

        noise_tex_future
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None)
            .unwrap();

        Self {
            noise_image: noise_tex,
        }
    }
}

impl ImageProducer for AONoiseTexProducer {
    fn create_image(&self, _device: Arc<Device>) -> Arc<dyn ImageViewAccess + Send + Sync> {
        self.noise_image.clone()
    }

    fn name(&self) -> &str {
        "ao_noise"
    }
}

struct DimensionsProducer {
    dimensions: DimensionsUniform,
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
struct DimensionsUniform {
    x: u32,
    y: u32,
}

impl DimensionsProducer {
    fn new() -> Self {
        Self {
            dimensions: DimensionsUniform { x: 0, y: 0 },
        }
    }
}

impl BufferProducer for DimensionsProducer {
    fn update(&mut self, frame_info: FrameInfo) {
        self.dimensions = DimensionsUniform {
            x: frame_info.dimensions[0],
            y: frame_info.dimensions[1],
        }
    }

    fn create_buffer(&self, device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
        let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<DimensionsUniform>::new(
            device.clone(),
            vulkano::buffer::BufferUsage::all(),
        );

        let uniform_data = self.dimensions;

        Arc::new(pool.next(uniform_data).unwrap())
    }

    fn name(&self) -> &str {
        "dimensions"
    }
}
