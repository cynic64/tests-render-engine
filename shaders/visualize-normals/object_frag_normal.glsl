#version 450

layout(location = 0) in vec3 v_pos;
layout(location = 1) in vec2 v_tex_coord;
layout(location = 2) in vec3 v_normal;
layout(location = 3) in vec3 v_tangent;
layout(location = 4) in vec3 v_bitangent;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Model {
    mat4 model;
} model;

layout(set = 0, binding = 1) uniform Camera {
    mat4 view;
    mat4 proj;
    vec3 pos;
} camera;

layout(set = 0, binding = 2) uniform sampler2D normal_tex;

void main() {
    f_color = vec4(v_normal * 0.5 + 0.5, 1.0);
}
