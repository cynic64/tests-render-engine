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

layout(set = 0, binding = 2) uniform Light {
    vec3 position;
    vec3 ambient;
    vec3 diffuse;
    vec3 specular;
} light;

layout(set = 0, binding = 3) uniform Material {
    float shininess;
} material;

layout(set = 0, binding = 4) uniform sampler2D diffuse_texture;
layout(set = 0, binding = 5) uniform sampler2D specular_texture;
layout(set = 0, binding = 6) uniform sampler2D normal_texture;

void main() {
    vec3 tex_diffuse = texture(diffuse_texture, v_tex_coord).rgb;
    vec3 tex_specular = texture(specular_texture, v_tex_coord).rgb;

    vec3 tex_normal = normalize(texture(normal_texture, v_tex_coord).rgb * 2.0 - 1.0);

    // ambient
    vec3 ambient = tex_diffuse * light.ambient;

    // diffuse
    vec3 light_dir = normalize(light.position - v_pos);

    float diff = max(dot(v_normal, light_dir), 0.0);
    vec3 diffuse = light.diffuse * (diff * tex_diffuse);

    // specular
    vec3 view_dir = normalize(camera.pos - v_pos);
    vec3 reflect_dir = reflect(-light_dir, v_normal);
    float spec = pow(max(dot(view_dir, reflect_dir), 0.0), material.shininess);
    vec3 specular = light.specular * (spec * tex_specular);

    // result
    vec3 result = ambient + diffuse + specular;

    f_color = vec4(result, 1.0);
}
