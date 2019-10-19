#version 450

layout(location = 0) in vec3 v_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_tex_coord;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Model {
    mat4 model;
} model;

layout(set = 0, binding = 1) uniform Camera {
    mat4 view;
    mat4 proj;
    vec3 pos;
} camera;

void main() {
    vec3 m_ambient = vec3(0.2);
    vec3 m_diffuse = vec3(0.7);

    // ambient
    vec3 ambient = m_diffuse * m_ambient;

    // diffuse
    vec3 light_dir = normalize(camera.pos - v_pos);

    float diff = max(dot(v_normal, light_dir), 0.0);
    vec3 diffuse = diff * m_diffuse;

    // result
    vec3 result = ambient + diffuse;

    f_color = vec4(result, 1.0);
}
