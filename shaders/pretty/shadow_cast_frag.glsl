#version 450

layout(location = 0) in vec3 v_pos;

void main() {
  // 0, 10, 0 is light's pos
  float light_dist = length(v_pos - vec3(0.0, 10.0, 0.0));

  // map to 0, 1 by dividing by far plane
  light_dist /= 250.0;

  gl_FragDepth = light_dist;
}
