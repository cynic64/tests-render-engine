#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 tex_coord;
layout(location = 2) in vec3 normal;
layout(location = 3) in vec3 tangent;

layout(location = 0) out vec3 v_pos;
layout(location = 1) out vec2 v_tex_coord;
layout(location = 2) out vec3 v_normal;
layout(location = 3) out vec3 v_tangent;

layout(set = 0, binding = 0) uniform Model {
    mat4 model;
} model;

layout(set = 0, binding = 1) uniform Camera {
    mat4 view;
    mat4 proj;
} camera;

layout(set = 0, binding = 2) uniform sampler2D normal_tex;

void main() {
     v_pos = vec3(model.model * vec4(position, 1.0));
     v_normal = normal;
     v_tangent = tangent;
     v_tex_coord = tex_coord;
     gl_Position = camera.proj * camera.view * vec4(v_pos, 1.0);
}