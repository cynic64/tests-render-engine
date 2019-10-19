#version 450

layout(location = 0) in vec3 v_pos;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec2 v_tex_coord;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D erato_texture;
layout(set = 1, binding = 0) uniform ViewProj {
    mat4 view;
    mat4 proj;
    vec3 pos;
} camera;

layout(set = 1, binding = 1) uniform LightInfo {
    vec3 position;
    vec3 ambient;
    vec3 diffuse;
    vec3 specular;
} light;

layout(set = 1, binding = 2) uniform Model {
    mat4 model;
} model;

layout(set = 1, binding = 3) uniform Material {
    vec3 ambient;
    vec3 diffuse;
    vec3 specular;
    vec3 shininess;
} material;

void main() {
    vec3 tex_diffuse = texture(erato_texture, v_tex_coord).rgb;

    // ambient
    vec3 ambient = tex_diffuse * light.ambient;

    // diffuse
    vec3 norm = normalize(v_normal);
    vec3 light_dir = normalize(light.position - v_pos);

    float diff = max(dot(v_normal, light_dir), 0.0);
    vec3 diffuse = light.diffuse * (diff * tex_diffuse);

    // specular
    float specular_strength = 0.5;
    vec3 view_dir = normalize(camera.pos - v_pos);
    vec3 reflect_dir = reflect(-light_dir, norm);
    float spec = pow(max(dot(view_dir, reflect_dir), 0.0), material.shininess.x);
    vec3 specular = light.specular * (spec * material.specular);

    // result
    vec3 result = ambient + diffuse + specular;

    f_color = vec4(result, 1.0);
    // f_color = texture(erato_texture, v_tex_coord);
}
