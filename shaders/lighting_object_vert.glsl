#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;

layout(location = 0) out vec3 v_pos;
layout(location = 1) out vec3 v_normal;

layout(set = 0, binding = 0) uniform ViewProj {
    mat4 view;
    mat4 proj;
} view_proj;

layout(set = 1, binding = 0) uniform Model {
    mat4 model;
} model;

void main() {
    gl_Position = view_proj.proj * view_proj.view * model.model * vec4(position, 1.0);
    v_pos = vec3(model.model * vec4(position, 1.0));
    v_normal = normal;
}
