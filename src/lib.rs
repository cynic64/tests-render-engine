use render_engine as re;

use re::input::FrameInfo;
use re::utils::bufferize_data;

// TODO: move default-sampler to re
use vulkano::buffer::BufferAccess;
use vulkano::device::{Device, Queue};
use vulkano::sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode};

use nalgebra_glm::*;

use std::path::PathBuf;
use std::sync::Arc;

pub mod mesh;

pub fn relative_path(local_path: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), local_path].iter().collect()
}

#[derive(Clone)]
pub struct OrbitCamera {
    pub center_position: Vec3,
    pub front: Vec3,
    up: Vec3,
    right: Vec3,
    world_up: Vec3,
    // pitch and yaw are in radians
    pub pitch: f32,
    pub yaw: f32,
    pub orbit_distance: f32,
    mouse_sens: f32,
    view_mat: CameraMatrix,
    proj_mat: CameraMatrix,
}

// TODO: builders for changing fov, perspective, orbit dist, etc.
impl OrbitCamera {
    pub fn default() -> Self {
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
        let orbit_distance = 20.0;

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

    pub fn update(&mut self, frame_info: FrameInfo) {
        // check for scroll wheel
        let scroll: f32 = frame_info.all_events.iter().map(|ev| match ev {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::MouseWheel {
                    delta: winit::MouseScrollDelta::LineDelta(_, y),
                    ..
                },
                ..
            } => {
                *y
            },
            _ => 0.0,
        }).sum();

        self.orbit_distance += scroll;

        // TODO: a lot of the stuff stored in OrbitCamera doesn't need to be
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
        // TODO: idk why i have to flip it vertically
        self.proj_mat = scale(
            &perspective(
                aspect_ratio,
                // fov
                1.0,
                // near
                0.1,
                // far
                10_000.,
            ),
            &vec3(1.0, -1.0, 1.0),
        )
        .into();
    }

    pub fn get_buffer(&self, queue: Arc<Queue>) -> Arc<dyn BufferAccess + Send + Sync> {
        bufferize_data(
            queue,
            CameraData {
                view: self.view_mat,
                proj: self.proj_mat,
                pos: (self.front * self.orbit_distance).into(),
            },
        )
    }
}

#[allow(dead_code)]
struct CameraData {
    view: CameraMatrix,
    proj: CameraMatrix,
    pos: [f32; 3],
}

pub type CameraMatrix = [[f32; 4]; 4];

pub fn default_sampler(device: Arc<Device>) -> Arc<Sampler> {
    Sampler::new(
        device,
        Filter::Linear,
        Filter::Linear,
        MipmapMode::Nearest,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::Repeat,
        0.0,
        1.0,
        0.0,
        0.0,
    )
    .unwrap()
}
