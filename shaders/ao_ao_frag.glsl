#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) uniform sampler2D g_position;
layout(set = 0, binding = 1) uniform sampler2D g_normal;
layout(set = 0, binding = 2) uniform sampler2D noise_tex;
layout(set = 1, binding = 0) uniform Data {
    mat4 view;
    mat4 projection;
} view_proj;
// TODO: try removing the unnecessary Data
layout(set = 1, binding = 1) uniform OtherData {
    vec4 samples[32];
} ao_samples;
layout(set = 1, binding = 2) uniform ThirdData {
    unsigned int[2] dimensions;
}

const float bias = 0.025;
const float radius = 0.5;

// tile noise texture over screen based on screen dimensions divided by noise size
const vec2 noise_scale = vec2(1856.0/4.0, 1016.0/4.0);

void main() {
    vec3 frag_pos = texture(g_position, tex_coords).xyz;
    vec3 normal = texture(g_normal, tex_coords).xyz;
    vec3 random_vec = normalize(texture(noise_tex, tex_coords * noise_scale).xyz);

    vec3 tangent   = normalize(random_vec - normal * dot(random_vec, normal));
    vec3 bitangent = cross(normal, tangent);
    mat3 TBN       = mat3(tangent, bitangent, normal);

    vec4 frag_offset = vec4(frag_pos, 1.0);
    frag_offset = view_proj.projection * frag_offset;
    frag_offset.xyz /= frag_offset.w;
    frag_offset.xyz = frag_offset.xyz * 0.5 + 0.5;
    float frag_depth = texture(g_position, frag_offset.xy).z;

    float occlusion = 0.0;
    for (int i = 0; i < 32; i++) {
        vec3 c_sample = TBN * ao_samples.samples[i].xyz;
        c_sample = frag_pos + c_sample * radius;

        vec4 offset = vec4(c_sample, 1.0);
        offset      = view_proj.projection * offset;    // from view to clip-space
        offset.xyz /= offset.w;               // perspective divide
        offset.xyz  = offset.xyz * 0.5 + 0.5; // transform to range 0.0 - 1.0

        float sample_depth = texture(g_position, offset.xy).z;
        float range_check = smoothstep(0.0, 1.0, radius / abs(frag_pos.z - sample_depth));

        occlusion += (sample_depth >= frag_depth + bias ? 1.0 : 0.0) * range_check;
    }

    occlusion = 1.0 - (occlusion / 32.0);
    f_color = vec4(occlusion, occlusion, occlusion, 1.0);
}
