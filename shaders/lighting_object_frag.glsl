#version 450

layout(location = 0) out vec4 f_color;

layout(set = 1, binding = 1) uniform LightInfo {
    vec3 light_color;
    vec3 object_color;
} light_info;

void main() {
    f_color = vec4(1.0, 1.0, 1.0, 1.0);
}