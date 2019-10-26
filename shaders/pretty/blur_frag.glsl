#version 450

layout(location = 0) in vec2 v_pos;

layout(set = 0, binding = 0) uniform sampler2D depth_map;

vec2 clamp_uv(vec2 orig, vec2 offset) {
  // makes sure the given uv coordinates doesn't cross one of the patch boundaries
  float margin = 0.001;

  if (orig.x <= 1.0 / 6.0) {
    return vec2(clamp(orig.x + offset.x, 0.0 + margin, 1.0 / 6.0 - margin), orig.y + offset.y);
  } else if (orig.x <= 2.0 / 6.0) {
    return vec2(clamp(orig.x + offset.x, 1.0 / 6.0 + margin, 2.0 / 6.0 - margin), orig.y + offset.y);
  } else if (orig.x <= 3.0 / 6.0) {
    return vec2(clamp(orig.x + offset.x, 2.0 / 6.0 + margin, 3.0 / 6.0 - margin), orig.y + offset.y);
  } else if (orig.x <= 4.0 / 6.0) {
    return vec2(clamp(orig.x + offset.x, 3.0 / 6.0 + margin, 4.0 / 6.0 - margin), orig.y + offset.y);
  } else if (orig.x <= 5.0 / 6.0) {
    return vec2(clamp(orig.x + offset.x, 4.0 / 6.0 + margin, 5.0 / 6.0 - margin), orig.y + offset.y);
  } else {
    return vec2(clamp(orig.x + offset.x, 5.0 / 6.0 + margin, 6.0 / 6.0 - margin), orig.y + offset.y);
  }
}

void main() {
  float depth = 0.0;
  float radius = 0.0005;
  for (int x = -2; x <= 2; x++) {
    for (int y = -2; y <= 2; y++) {
      vec2 tex_coords = clamp_uv(v_pos.xy, vec2(x * radius, y * radius));
      float sample_depth = texture(depth_map, tex_coords).r;
      depth += sample_depth;
    }
  }
  gl_FragDepth = depth / 25.0;
}
