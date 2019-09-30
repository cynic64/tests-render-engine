#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D ssao;
layout(set = 0, binding = 1) uniform sampler2D g_color;

void main() {
    float occlusion = texture(ssao, tex_coords).x;
    vec3 albedo = texture(g_color, tex_coords).xyz;
    f_color = vec4(albedo * occlusion, 1.0);
}
