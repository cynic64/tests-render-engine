use render_engine as re;

use re::producer::{BufferProducer, ProducerCollection};
use re::world::ObjectSpecBuilder;
use re::{mesh_gen, App};
use re::input::{VirtualKeyCode, FrameInfo};

use vulkano::device::Device;
use vulkano::buffer::BufferAccess;

use nalgebra_glm as glm;

use std::path::PathBuf;
use std::sync::Arc;

const LIGHT_POS: [f32; 4] = [10.0, 2.0, 5.0, 1.0];

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
    let camera = Camera::default();
    let producers = ProducerCollection::new(vec![], vec![Box::new(camera)]);
    app.set_producers(producers);

    // main loop
    while !app.done {
        let frame_info = app.get_frame_info();
        if frame_info.keydowns.contains(&VirtualKeyCode::Escape) {
            break;
        }

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
        light_pos: LIGHT_POS,
        light_color: [1.0, 1.0, 1.0, 1.0],
        object_color: [1.0, 0.5, 0.31, 1.0],
        final_color: [1.0, 0.0, 1.0, 1.0],
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

#[allow(dead_code)]
struct LightInfo {
    light_pos: [f32; 4],
    light_color: [f32; 4],
    object_color: [f32; 4],
    final_color: [f32; 4],
}

type Matrix = [[f32; 4]; 4];

struct Camera {
    pub center_position: glm::Vec3,
    pub front: glm::Vec3,
    up: glm::Vec3,
    right: glm::Vec3,
    world_up: glm::Vec3,
    // pitch and yaw are in radians
    pub pitch: f32,
    pub yaw: f32,
    pub orbit_distance: f32,
    mouse_sens: f32,
    view_mat: CameraMatrix,
    proj_mat: CameraMatrix,
}

// TODO: builders for changing fov, perspective, orbit dist, etc.
impl Camera {
    pub fn default() -> Self {
        use glm::*;

        let center_position = vec3(0.0, 0.0, 0.0);
        let pitch: f32 = 0.0;
        let yaw: f32 = std::f32::consts::PI / 2.0;
        let front = normalize(&vec3(
            pitch.cos() * yaw.cos(),
            pitch.sin(),
            pitch.cos() * yaw.sin(),
        ));
        let right = vec3(0.0, 0.0, 0.0);
        let up = vec3(0.0, 1.0, 0.0);
        let world_up = vec3(0.0, 1.0, 0.0);
        let mouse_sens = 0.0007;
        let orbit_distance = 24.0;

        let view_mat: CameraMatrix = Mat4::identity().into();
        let proj_mat: CameraMatrix = Mat4::identity().into();

        Self {
            center_position,
            front,
            up,
            right,
            world_up,
            pitch,
            yaw,
            orbit_distance,
            mouse_sens,
            view_mat,
            proj_mat,
        }
    }
}

impl BufferProducer for Camera {
    fn update(&mut self, frame_info: FrameInfo) {
        use glm::*;

        // TODO: a lot of the stuff stored in Camera doesn't need to be
        // stored across frames
        let x = frame_info.mouse_movement[0];
        let y = frame_info.mouse_movement[1];

        self.pitch += y * self.mouse_sens;
        self.yaw += x * self.mouse_sens;
        let halfpi = std::f32::consts::PI / 2.0;
        let margin = 0.01;
        let max_pitch = halfpi - margin;

        if self.pitch > max_pitch {
            self.pitch = max_pitch;
        } else if self.pitch < -max_pitch {
            self.pitch = -max_pitch;
        }

        // recompute front vector
        self.front = normalize(&vec3(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.sin(),
            self.pitch.cos() * self.yaw.sin(),
        ));

        self.right = normalize(&Vec3::cross(&self.front, &self.world_up));

        // recompute view and projection matrices
        let farther_front = self.front * self.orbit_distance;
        self.view_mat = look_at(
            &(self.center_position + farther_front),
            &self.center_position,
            &self.up,
        )
        .into();

        let dims = frame_info.dimensions;
        let aspect_ratio = (dims[0] as f32) / (dims[1] as f32);
        self.proj_mat = perspective(
            aspect_ratio,
            // fov
            1.0,
            // near
            0.1,
            // far
            100_000_000.,
        )
        .into();
    }

    fn create_buffer(&self, device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
        let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<CameraData>::new(
            device.clone(),
            vulkano::buffer::BufferUsage::all(),
        );

        let data = CameraData {
            view: self.view_mat,
            proj: self.proj_mat,
            pos: (self.front * self.orbit_distance).into(),
        };
        Arc::new(pool.next(data).unwrap())
    }

    fn name(&self) -> &str {
        "view_proj"
    }
}

type CameraMatrix = [[f32; 4]; 4];

#[allow(dead_code)]
struct CameraData {
    view: CameraMatrix,
    proj: CameraMatrix,
    pos: [f32; 3],
}
