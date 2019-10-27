#version 450

layout(location = 0) in vec2 v_tex_coord;
layout(location = 1) in vec3 tan_light_pos;
layout(location = 2) in vec3 tan_cam_pos;
layout(location = 3) in vec3 tan_frag_pos;
layout(location = 4) in vec3 v_pos;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D shadow_map;
layout(set = 0, binding = 1) uniform sampler2D depth_map;
layout(set = 1, binding = 0) uniform Material {
  vec3 ambient;
  vec3 diffuse;
  vec3 specular;
  vec3 shininess;
  vec3 use_texture;
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

float A = 0.15;
float B = 0.50;
float C = 0.10;
float D = 0.20;
float E = 0.02;
float F = 0.30;
float W = 11.2;

const vec3 thingy[32]=vec3[](
                              vec3(0.38778852289282234, 0.503137111927922, 0.7723167148996792),
                              vec3(0.7816915355448285, 0.6232594169504354, -0.022495387086500004),
                              vec3(0.713004609048526, -0.6427054830727796, -0.2802750247591525),
                              vec3(0.6906581990652374, 0.5940971589073923, -0.4123588459608058),
                              vec3(0.6657849452084662, -0.4917531048688565, 0.5611677918284326),
                              vec3(-0.3418525811474961, -0.9209933713595635, -0.18683688788496755),
                              vec3(-0.7708487274458536, 0.00533988216625939, 0.6369958595262283),
                              vec3(-0.9191704500543458, 0.3159303949339045, 0.23518858242652524),
                              vec3(-0.998910436891013, 0.03873380890617433, -0.026031348751558898),
                              vec3(0.5037356467899858, -0.7383357409015021, -0.4484537120601238),
                              vec3(0.8393759936679026, 0.3041145332275624, 0.4505133648781167),
                              vec3(0.18787150199301167, 0.8291846452592665, -0.5264571424201159),
                              vec3(-0.8813271169433724, -0.37401304677184577, 0.2887503312287696),
                              vec3(0.9061833911896379, -0.3391526053897629, -0.252600815100394),
                              vec3(-0.372376326252011, -0.5956796672096547, 0.711689262051951),
                              vec3(0.33988633625685255, -0.5038628637699192, -0.7941029485774935),
                              vec3(-0.4930014390621104, -0.8601477913445905, -0.13074921845927423),
                              vec3(-0.28416677832604154, 0.5264857720393369, -0.8012876973571613),
                              vec3(-0.8917748786755901, 0.392899824048683, -0.2244265893909418),
                              vec3(-0.5469591896697178, -0.5767080089658572, -0.6068307154639441),
                              vec3(-0.11744798105892841, 0.49688800794526133, -0.8598303782173385),
                              vec3(0.16973855972615212, 0.5158771859491886, 0.8396782421613874),
                              vec3(0.28861815162170906, 0.8665791155940756, -0.4071120226309435),
                              vec3(0.7779105189949913, -0.28705225112182264, -0.5589778435348198),
                              vec3(-0.9866822417881215, -0.07294146615729329, 0.14538808842127116),
                              vec3(-0.009006347474779216, 0.7160198386766499, 0.6980218308381393),
                              vec3(0.8112807914257281, -0.3134163524293244, 0.4935520919756515),
                              vec3(0.6780136766609725, -0.7267940492726386, -0.10985383107817183),
                              vec3(-0.35207401433306695, -0.3961647723710566, -0.8479960858185539),
                              vec3(-0.36142125543323605, 0.6567702593005538, 0.6618364621410238),
                              vec3(-0.2721665688253435, 0.837351299099321, -0.4740972059720651),
                              vec3(0.8021147768180403, -0.5315897308698458, 0.2720739657590743)
                              );

// taken from: http://filmicworlds.com/blog/filmic-tonemapping-operators/
vec3 Uncharted2Tonemap(vec3 x)
{
  return ((x*(A*x+C*B)+D*E)/(x*(A*x+B)+D*F))-E/F;
}

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
  uv = uv * 0.9921875 + 0.00390625;
  uv.x = (uv.x + face_index) / 6.f;
  return uv;
}

float shadowedness() {
  vec3 light_dir = normalize(v_pos - light.position);
  vec2 coords = l_to_shadow_map_uv(light_dir);
  float sample_dist = texture(shadow_map, coords).r * 250.0;

  float frag_dist = length(v_pos - light.position);
  float bias = 0.05;

  // idk why i have to invert it
  float difference = abs(sample_dist - frag_dist);

  return clamp(difference, 0.0, 1.0);
  /* return !(sample_dist + bias > frag_dist); */
}

float get_occlusion() {
  float occl = 0.0;
  float radius = 1.0;

  // for debugging
  float avg_sample = 0.0;
  float avg_tex = 0.0;

  for (int i = 0; i < 32; i++) {
    vec3 a_sample = thingy[i];
    a_sample = v_pos + a_sample * radius;

    vec4 screen_space = camera.proj * camera.view * vec4(a_sample, 1.0);
    screen_space.xyz /= screen_space.w;
    avg_sample += screen_space.z;

    vec2 tex_coords = screen_space.xy * 0.5 + 0.5;
    float sampled_depth = texture(depth_map, tex_coords).r;
    avg_tex += sampled_depth;

    if (sampled_depth < screen_space.z) {
        occl += 1.0;
    }
  }

  occl /= 32.0;
  // subtract .5 and clamp because we use a spherical sample thingy and half of
  // the samples end up blocked even on a flat wall
  occl -= 0.5;
  occl *= 4.0;
  occl = clamp(occl, 0.0, 1.0);

  avg_sample /= 32.0;
  avg_tex /= 32.0;

  return occl;
}

void main() {
  // only use the texture if we should
  vec4 tex_diffuse = material.use_texture.r > 0.5 ? texture(diffuse_map, v_tex_coord) : vec4(material.diffuse, 1.0);

  // doesn't play nice with depth prepass
  /*
  if (tex_diffuse.a < 0.5) {
    discard;
  }
  */

  vec3 tex_specular = texture(specular_map, v_tex_coord).rgb;

  vec3 normal = texture(normal_map, v_tex_coord).rgb * 2.0 - 1.0;

  // ambient
  vec3 ambient = tex_diffuse.rgb * 0.01;

  // diffuse
  vec3 light_dir = normalize(tan_light_pos - tan_frag_pos);

  float diff = max(dot(normal, light_dir), 0.0);
  vec3 diffuse = diff * tex_diffuse.rgb;

  // specular
  vec3 view_dir = normalize(tan_cam_pos - tan_frag_pos);
  vec3 halfway_dir = normalize(light_dir + view_dir);
  float spec = pow(max(dot(normal, halfway_dir), 0.0), 32.0);
  /* vec3 specular = material.specular * spec; */
  vec3 specular = vec3(clamp(0.2 * spec, 0.0, 0.5));

  // result
  /* vec3 result = ambient + (diffuse + specular) * light.strength.r; */
  float dist = length(tan_light_pos - tan_frag_pos);
  /* float shadow = shadowedness(); */
  float shadow = 0.0;
  float ao = get_occlusion();

  vec3 result = ambient + (1.0 - shadow) * (diffuse + specular) * light.strength.r / (dist * dist / 2000.0);
  result *= (1.0 - ao);

  // gamma correction and reinhard
  /*
  vec3 mapped = result / (result + vec3(1.0));
  float gamma = 2.2;
  vec3 corrected = pow(mapped, vec3(1.0/gamma));
  */

  // uncharted 2 tone mapping
  result *= 16;
  float exposure_bias = 2.0;
  vec3 curr = Uncharted2Tonemap(exposure_bias * result);

  /*
  vec3 white_scale = 1.0 / Uncharted2Tonemap(vec3(W));
  vec3 color = curr * white_scale;
  */

  vec3 corrected = pow(curr, vec3(1/2.2));

  /* f_color = vec4(vec3(get_occlusion()), 1.0); */
  f_color = vec4(corrected, 1.0);
}
