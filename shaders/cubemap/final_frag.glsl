#version 450

layout(location = 0) in vec3 v_pos;
layout(location = 1) in vec3 v_normal;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D shadow_map;

// cube faces +x, -x, +y, -y, +z, -z in a row
// taken from: http://blue2rgb.sydneyzh.com/rendering-dynamic-cube-maps-for-omni-light-shadows-with-vulkan-api.html
vec2 l_to_shadow_map_uv(vec3 v) {
  float face_index;
  vec3 v_abs = abs(v);
  float ma;
  vec2 uv;
  if(v_abs.z >= v_abs.x && v_abs.z >= v_abs.y)
    {
      face_index = v.z < 0.0 ? 5.0 : 4.0;
      ma = 0.5 / v_abs.z;
      uv = vec2(v.z < 0.0 ? -v.x : v.x, -v.y);
    }
  else if(v_abs.y >= v_abs.x)
    {
      face_index = v.y < 0.0 ? 3.0 : 2.0;
      ma = 0.5 / v_abs.y;
      uv = vec2(v.x, v.y < 0.0 ? -v.z : v.z);
    }
  else
    {
      face_index = v.x < 0.0 ? 1.0 : 0.0;
      ma = 0.5 / v_abs.x;
      uv = vec2(v.x < 0.0 ? v.z : -v.z, -v.y);
    }
  uv = uv * ma + 0.5;
  uv.x = (uv.x + face_index) / 6.f;
  return uv;
}

void main() {
  // light is in center
  vec3 light_dir = normalize(v_pos);
  vec2 coords = l_to_shadow_map_uv(light_dir);
  float depth = texture(shadow_map, coords).r;
  vec3 depth_comp = vec3(pow(depth, 20.0));
  f_color = vec4(normalize(v_normal + depth_comp), 1.0);
}
