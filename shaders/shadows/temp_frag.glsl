#version 450

layout(location = 0) in vec2 v_tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D depth_tex;

void main() {
  float depth = texture(depth_tex, v_tex_coords).r;
  f_color = vec4(vec3(pow(depth, 100.0)), 1.0);
}
