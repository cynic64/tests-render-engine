#version 450

layout(location = 0) in vec3 v_pos;
layout(location = 1) in vec3 v_normal;

layout(location = 0) out vec4 f_color;

// for some reason including this is necessary to make the
// second uniform work. what the hell, GLSL.
layout(set = 1, binding = 0) uniform Model {
    mat4 model;
} model;

// AND FOR SOME REASON I CAN'T USE VEC3??
// that might be vulkano's fault though
layout(set = 1, binding = 1) uniform LightInfo {
    vec4 l_pos;
    vec4 l_color;
    vec4 o_color;
} light_info;

void main() {
    vec3 light_pos = light_info.l_pos.xyz;
    vec3 light_color = light_info.l_color.xyz;
    vec3 object_color = light_info.o_color.xyz;

    // ambient
    float ambient_strength = 0.1;
    vec3 ambient = ambient_strength * light_color;

    // diffuse
    vec3 norm = normalize(v_normal);
    vec3 light_dir = normalize(light_pos - v_pos);

    float diff = max(dot(v_normal, light_dir), 0.0);
    vec3 diffuse = diff * light_color;

    vec3 result = (ambient + diffuse) * object_color;

    f_color = vec4(result, 1.0);
}
