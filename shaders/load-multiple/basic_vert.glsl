#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 tex_coord;
layout(location = 2) in vec3 normal;

layout(location = 0) out vec3 v_pos;
layout(location = 1) out vec2 v_tex_coord;
layout(location = 2) out vec3 v_normal;

layout(set = 0, binding = 0) uniform Material {
  vec3 ambient;
  vec3 diffuse;
  vec3 specular;
  vec3 shininess;
} material;

layout(set = 0, binding = 1) uniform Model {
  mat4 model;
} model;

layout(set = 1, binding = 0) uniform sampler2D diffuse_map;

layout(set = 2, binding = 0) uniform Camera {
  mat4 view;
  mat4 proj;
  vec3 pos;
} camera;

void main() {
  v_pos = vec3(model.model * vec4(position, 1.0));
  gl_Position = camera.proj * camera.view * vec4(v_pos, 1.0);
  v_tex_coord = tex_coord;
  v_normal = normal;
}
