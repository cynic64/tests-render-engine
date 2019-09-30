#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 f_occlusion;

layout(set = 0, binding = 0) uniform sampler2D g_position;
layout(set = 0, binding = 1) uniform sampler2D g_normal;
layout(set = 0, binding = 2) uniform sampler2D noise_tex;
layout(set = 1, binding = 0) uniform Data {
    mat4 view;
    mat4 projection;
} view_proj;
// TODO: try removing the unnecessary Data
layout(set = 1, binding = 1) uniform OtherData {
    vec4 samples[64];
} ao_samples;
layout(set = 1, binding = 2) uniform ThirdData {
    uint x;
    uint y;
} dimensions;

const float bias = 0.025;
const float radius = 0.5;

void main() {
    vec2 noise_scale = vec2(dimensions.x / 4.0, dimensions.y / 4.0);

    // get input for SSAO algorithm
    vec3 frag_pos = texture(g_position, tex_coords).xyz;
    vec3 normal = normalize(texture(g_normal, tex_coords).rgb);
    vec3 random_vec = normalize(texture(noise_tex, tex_coords * noise_scale).xyz);
    // create TBN change-of-basis matrix: from tangent-space to view-space
    vec3 tangent = normalize(random_vec - normal * dot(random_vec, normal));
    vec3 bitangent = cross(normal, tangent);
    mat3 TBN = mat3(tangent, bitangent, normal);
    // iterate over the sample kernel and calculate occlusion factor
    float occlusion = 0.0;
    for(int i = 0; i < 64; ++i) {
        // get sample position
        vec3 c_sample = TBN * ao_samples.samples[i].xyz; // from tangent to view-space
        c_sample = frag_pos + c_sample * radius;

        // project sample position (to sample texture) (to get position on screen/texture)
        vec4 offset = vec4(c_sample, 1.0);
        offset = view_proj.projection * offset; // from view to clip-space
        offset.xyz /= offset.w; // perspective divide
        offset.xyz = offset.xyz * 0.5 + 0.5; // transform to range 0.0 - 1.0

        // get sample depth
        float sample_depth = texture(g_position, offset.xy).z; // get depth value of kernel sample

        // range check & accumulate
        float range_check = smoothstep(0.0, 1.0, radius / abs(frag_pos.z - sample_depth));
        occlusion += (sample_depth >= c_sample.z + bias ? 1.0 : 0.0) * range_check;
    }
    occlusion = 1.0 - (occlusion / 64.0);

    f_occlusion = vec4(occlusion, occlusion, occlusion, 1.0);
}
