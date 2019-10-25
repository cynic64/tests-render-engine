#version 450

layout(location = 0) in vec3 position;

layout(set = 0, binding = 0) uniform Model {
    mat4 model;
} model;

layout(set = 1, binding = 0) uniform Proj {
    mat4 proj;
} shadow_proj;

layout(set = 2, binding = 0) uniform View {
  mat4 view;
} shadow_view;

void main() {
  gl_Position = shadow_proj.proj * shadow_view.view * model.model * vec4(position, 1.0);
}
