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
    vec3 m_ambient = vec3(0.2);
    vec3 m_diffuse = vec3(0.7);

    vec3 norm = normalize(v_normal);
    vec3 tangent = normalize(v_tangent);
    vec3 bitangent = normalize(v_bitangent);
    // mat3 TBN = mat3(tangent, bitangent, norm);
    mat3 TBN = mat3(tangent, bitangent, norm);

    vec3 normal = texture(normal_tex, v_tex_coord).rgb;
    normal = normalize(normal * 2.0 - 1.0);
    normal = normalize(TBN * normal);

    // ambient
    vec3 ambient = m_diffuse * m_ambient;

    // diffuse
    vec3 light_dir = normalize(camera.pos - v_pos);

    float diff = max(dot(v_normal, light_dir), 0.0);
    vec3 diffuse = diff * m_diffuse;

    // result
    vec3 result = ambient + diffuse;

    f_color = vec4(v_normal * 0.5 + 0.5, 1.0);
}
