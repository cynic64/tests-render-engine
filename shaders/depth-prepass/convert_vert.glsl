#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec2 v_tex_coords;

layout(set = 0, binding = 0) uniform sampler2D depth_tex;

void main() {
  gl_Position = vec4(tex_coords, 0.0, 1.0);
  v_tex_coords = tex_coords * 0.5 + 0.5;
}