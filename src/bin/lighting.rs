use render_engine as re;

use re::input::{get_elapsed, FrameInfo, VirtualKeyCode};
use re::producer::{BufferProducer, ProducerCollection};
use re::render_passes;
use re::system::{Pass, System, Vertex};
use re::world::{Mesh, ObjectSpecBuilder};
use re::{mesh_gen, App};

use vulkano::buffer::BufferAccess;
use vulkano::device::Device;

use nalgebra_glm as glm;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

const EMERALD: Material = Material {
    ambient: [0.0215, 0.1745, 0.0215, 0.0],
    diffuse: [0.07568, 0.61424, 0.07568, 0.0],
    specular: [0.633, 0.727811, 0.633, 0.0],
    shininess: 76.8,
};

const BRASS: Material = Material {
    ambient: [0.329412, 0.223529, 0.027451, 0.0],
    diffuse: [0.780392, 0.568627, 0.113725, 0.0],
    specular: [0.992157, 0.941176, 0.807843, 0.0],
    shininess: 26.88,
};

fn main() {
    // paths and loading meshes
    let dragon_path = relative_path("meshes/dragon.obj");
    let dragon_mesh = mesh_gen::load_obj(&dragon_path).unwrap();
    let dragon_mesh = load_obj(&dragon_path);

    let happy_path = relative_path("meshes/happy.obj");
    let happy_mesh = mesh_gen::load_obj(&happy_path).unwrap();

    let vs_path = relative_path("shaders/lighting_object_vert.glsl");
    let object_fs_path = relative_path("shaders/lighting_object_frag.glsl");
    let light_fs_path = relative_path("shaders/lighting_light_frag.glsl");

    // app init
    let mut app = App::new();
    let mut world_com = app.get_world_com();

    // creating buffers
    let dragon_matrix: Matrix = glm::Mat4::identity().into();
    let happy_matrix: Matrix = glm::translate(
        &glm::rotate(
            &glm::scale(&glm::Mat4::identity(), &glm::vec3(1.0, 0.5, 1.0)),
            std::f32::consts::PI / 2.0,
            &glm::vec3(1.0, 1.0, 1.0),
        ),
        &glm::vec3(0.0, 20.0, 3.0),
    )
    .into();
    let dragon_matrix_buffer = bufferize(app.get_device(), dragon_matrix);
    let happy_matrix_buffer = bufferize(app.get_device(), happy_matrix);

    let dragon_material_buffer = bufferize(app.get_device(), EMERALD);
    let happy_material_buffer = bufferize(app.get_device(), BRASS);

    // adding objects
    let dragon_spec = ObjectSpecBuilder::default()
        .mesh(dragon_mesh)
        .shaders(vs_path.clone(), object_fs_path.clone())
        .additional_resources(vec![dragon_matrix_buffer, dragon_material_buffer.clone()])
        .build();
    let happy_spec = ObjectSpecBuilder::default()
        .mesh(happy_mesh)
        .shaders(vs_path.clone(), object_fs_path.clone())
        .additional_resources(vec![happy_matrix_buffer, happy_material_buffer])
        .build();
    world_com.add_object_from_spec("dragon", dragon_spec);
    world_com.add_object_from_spec("happy", happy_spec);

    // change camera to one with a farther orbit distance
    let camera = Camera::default();
    let (light_info_p, light_pos_recv) = LightInfoProducer::new();
    let producers = ProducerCollection::new(vec![], vec![Box::new(camera), Box::new(light_info_p)]);
    app.set_producers(producers);

    // change system to include light info
    let system = {
        let pass = Pass::Complex {
            name: "geometry",
            images_needed: vec![],
            images_created: vec!["color", "depth"],
            buffers_needed: vec!["view_proj", "light_info"],
            render_pass: render_passes::with_depth(app.get_device()),
        };

        let output_tag = "color";
        System::new(app.get_queue(), vec![pass], output_tag)
    };
    app.set_system(system);

    let mut light_pos = [0.0, 0.0, 0.0];
    // main loop
    while !app.done {
        let frame_info = app.get_frame_info();
        if frame_info.keydowns.contains(&VirtualKeyCode::Escape) {
            break;
        }

        // update light
        if let Some(pos) = light_pos_recv.try_iter().last() {
            light_pos = pos;
        }

        let light_matrix: [[f32; 4]; 4] = glm::translate(
            &glm::Mat4::identity(),
            &glm::vec3(light_pos[0], light_pos[1], light_pos[2]),
        )
        .into();
        let light_matrix_buffer = bufferize(app.get_device(), light_matrix);
        let light_spec = ObjectSpecBuilder::default()
            .shaders(vs_path.clone(), light_fs_path.clone())
            .additional_resources(vec![light_matrix_buffer, dragon_material_buffer.clone()])
            .build();
        world_com.add_object_from_spec("light", light_spec);

        app.draw_frame();

        world_com.delete_object("light");
    }

    app.print_fps();
}

fn bufferize<T: vulkano::memory::Content + 'static + Send + Sync>(
    device: Arc<Device>,
    data: T,
) -> Arc<dyn BufferAccess + Send + Sync> {
    let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<T>::new(
        device.clone(),
        vulkano::buffer::BufferUsage::all(),
    );

    Arc::new(pool.next(data).unwrap())
}

fn relative_path(local_path: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), local_path].iter().collect()
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
struct Material {
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
    shininess: f32,
}

struct LightInfoProducer {
    light_info: LightInfo,
    start_time: std::time::Instant,
    orbit_distance: f32,
    light_pos_send: Sender<[f32; 3]>,
}

impl LightInfoProducer {
    fn new() -> (Self, Receiver<[f32; 3]>) {
        let (send, recv) = channel();

        let light_info = LightInfo {
            position: [0.0, 0.0, 0.0, 0.0],
            ambient: [0.2, 0.2, 0.2, 0.0],
            diffuse: [0.5, 0.5, 0.5, 0.0],
            specular: [1.0, 1.0, 1.0, 0.0],
        };

        let light_info_p = Self {
            light_info,
            start_time: std::time::Instant::now(),
            orbit_distance: 30.0,
            light_pos_send: send,
        };

        (light_info_p, recv)
    }
}

impl BufferProducer for LightInfoProducer {
    fn create_buffer(&self, device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
        let time = get_elapsed(self.start_time);
        let x = time.sin() * self.orbit_distance;
        let z = time.cos() * self.orbit_distance;
        let pos = [x, 0.0, z];
        self.light_pos_send.send(pos).unwrap();

        let data = LightInfo {
            position: [x, 0.0, z, 0.0],
            ..self.light_info
        };

        let pool = vulkano::buffer::cpu_pool::CpuBufferPool::<LightInfo>::new(
            device.clone(),
            vulkano::buffer::BufferUsage::all(),
        );

        Arc::new(pool.next(data).unwrap())
    }

    fn name(&self) -> &str {
        "light_info"
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
struct LightInfo {
    position: [f32; 4],
    ambient: [f32; 4],
    diffuse: [f32; 4],
    specular: [f32; 4],
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

        self.pitch -= y * self.mouse_sens;
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
        // TODO: i don't get why i need to flip this upside down
        self.proj_mat = scale(
            &perspective(
                aspect_ratio,
                // fov
                1.0,
                // near
                0.1,
                // far
                100_000_000.,
            ),
            &vec3(1.0, -1.0, 1.0),
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

fn load_obj(path: &Path) -> Mesh {
    dbg![&path];
    let (models, _materials) = tobj::load_obj(path).expect("Couldn't load OBJ file");

    // only use first mesh
    let mesh = &models[0].mesh;
    let mut vertices: Vec<Vertex> = vec![];

    for i in 0..mesh.positions.len() / 3 {
        let pos = [mesh.positions[i * 3], mesh.positions[i * 3 + 1], mesh.positions[i * 3 + 2]];
        let normal = [mesh.normals[i * 3], mesh.normals[i * 3 + 1], mesh.normals[i * 3 + 2]];
        let vertex = Vertex {
            position: pos,
            normal,
        };

        vertices.push(vertex);
    }

    Mesh {
        vertices: Box::new(vertices),
        indices: mesh.indices.clone(),
    }
}
