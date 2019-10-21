#version 450

layout(location = 0) in vec3 v_pos;
layout(location = 1) in vec2 v_tex_coord;
layout(location = 2) in vec3 v_normal;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform Model {
  mat4 model;
} model;

layout(set = 1, binding = 0) uniform Camera {
  mat4 view;
  mat4 proj;
  vec3 pos;
} camera;

/*
layout(set = 0, binding = 2) uniform Material {
  vec3 ambient;
  vec3 diffuse;
  vec3 specular;
  float shininess;
};
*/

void main() {
  vec3 ambient = vec3(0.2, 0.2, 0.2);
  vec3 m_diffuse = vec3(0.6, 0.6, 0.6);
  vec3 m_specular = vec3(1.0, 1.0, 1.0);

  // diffuse
  vec3 light_dir = normalize(camera.pos - v_pos);

  float diff = max(dot(v_normal, light_dir), 0.0);
  vec3 diffuse = diff * m_diffuse;

  // specular
  vec3 view_dir = normalize(camera.pos - v_pos);
  vec3 reflect_dir = reflect(-light_dir, v_normal);
  float spec = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0);
  vec3 specular = spec * m_specular;

  // result
  vec3 result = ambient + diffuse + specular;

  f_color = vec4(result, 1.0);
}
