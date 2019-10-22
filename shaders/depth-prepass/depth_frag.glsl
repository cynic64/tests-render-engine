#version 450

layout(set = 0, binding = 0) uniform Model {
  mat4 model;
} model;

layout(set = 1, binding = 0) uniform Camera {
  mat4 view;
  mat4 proj;
} camera;

layout(set = 1, binding = 1) uniform Light {
  vec3 position;
  vec3 strength;
} light;

void main() {
}
