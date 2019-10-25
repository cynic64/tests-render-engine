#version 450

layout(location = 0) in vec3 v_pos;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D depth_map;

void main() {
  float depth = texture(depth_map, v_pos.xy * 0.5 + 0.5).r;
  f_color = vec4(vec3(depth), 1.0);
  /* f_color = vec4(v_pos, 1.0); */
}


