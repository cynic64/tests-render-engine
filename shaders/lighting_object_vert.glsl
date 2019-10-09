#version 450

layout(location = 0) in vec3 position;

layout(set = 0, binding = 0) uniform ViewProj {
    mat4 view;
    mat4 proj;
} view_proj;

layout(set = 1, binding = 0) uniform Model {
    mat4 model;
} model;

void main() {
    gl_Position = view_proj.proj * view_proj.view * model.model * vec4(position, 1.0);
}