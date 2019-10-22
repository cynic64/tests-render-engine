#version 450

layout(location = 0) in vec2 v_tex_coord;
layout(location = 1) in vec3 tan_light_pos;
layout(location = 2) in vec3 tan_cam_pos;
layout(location = 3) in vec3 tan_frag_pos;
layout(location = 4) in vec4 frag_pos_light_space;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D shadow_map;

layout(set = 1, binding = 0) uniform Material {
  vec3 ambient;
  vec3 diffuse;
  vec3 specular;
  vec3 shininess;
} material;

layout(set = 1, binding = 1) uniform Model {
  mat4 model;
} model;

layout(set = 2, binding = 0) uniform sampler2D diffuse_map;
layout(set = 2, binding = 1) uniform sampler2D specular_map;
layout(set = 2, binding = 2) uniform sampler2D normal_map;

layout(set = 3, binding = 0) uniform Camera {
  mat4 view;
  mat4 proj;
  vec3 pos;
} camera;

layout(set = 3, binding = 1) uniform Light {
  vec3 position;
  vec3 strength; // vec3 really means float, idk why it doesn't work
} light;

float shadow_calculation(vec4 frag_pos_light_space) {
  // perspective divide
  vec3 proj_coords = frag_pos_light_space.xyz / frag_pos_light_space.w;
  // convert from -1 .. 1 to 0 .. 1
  proj_coords.xy = proj_coords.xy * 0.5 + 0.5;

  float closest_depth = texture(shadow_map, proj_coords.xy).r;
  float current_depth = proj_coords.z;

  float shadow = current_depth > closest_depth + 0.0001 ? 1.0 : 0.0;

  return shadow;
}

void main() {
  vec4 tex_diffuse = texture(diffuse_map, v_tex_coord);
  vec3 tex_specular = texture(specular_map, v_tex_coord).rgb;
  if (tex_diffuse.a < 0.5) {
    discard;
  }

  vec3 normal = texture(normal_map, v_tex_coord).rgb * 2.0 - 1.0;

  // ambient
  vec3 ambient = tex_diffuse.rgb * 0.05;

  // diffuse
  vec3 light_dir = normalize(tan_light_pos - tan_frag_pos);

  float diff = max(dot(normal, light_dir), 0.0);
  vec3 diffuse = diff * tex_diffuse.rgb;

  // specular
  vec3 view_dir = normalize(tan_cam_pos - tan_frag_pos);
  vec3 halfway_dir = normalize(light_dir + view_dir);
  float spec = pow(max(dot(normal, halfway_dir), 0.0), material.shininess.r);
  vec3 specular = material.specular * spec;

  // combine & shadowing
  float dist = length(tan_light_pos - tan_frag_pos);
  vec3 diff_spec = (diffuse + specular) / (dist * dist / 500.0) * light.strength.r;
  float shadow = shadow_calculation(frag_pos_light_space);
  vec3 result = ambient + (1.0 - shadow) * diff_spec;

  // gamma correction
  float gamma = 2.2;
  result.rgb = pow(result.rgb, vec3(1.0/gamma));

  f_color = vec4(vec3(shadow), 1.0);
}
