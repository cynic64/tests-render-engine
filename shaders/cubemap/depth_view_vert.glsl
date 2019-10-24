#version 450

layout(location = 0) in vec3 position;
layout(location = 0) out vec3 v_pos;

layout(set = 0, binding = 0) uniform sampler2D depth_map;

layout(set = 1, binding = 0) uniform Camera {
  mat4 view;
  mat4 proj;
} camera;

void main() {
  v_pos = position;
  gl_Position = camera.proj * camera.view * vec4(position, 1.0);
}
