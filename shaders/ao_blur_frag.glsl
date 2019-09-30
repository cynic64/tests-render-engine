#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out float occlusion;

layout(set = 0, binding = 0) uniform sampler2D ssao;

void main() {
    vec2 texel_size = 1.0 / vec2(textureSize(ssao, 0));
    float result = 0.0;
    for (int x = -3; x < 2; ++x) {
        for (int y = -2; y < 2; ++y) {
            vec2 offset = vec2(float(x), float(y)) * texel_size;
            result += texture(ssao, tex_coords + offset).r;
        }
    }
    occlusion = result / (4.0 * 4.0);
}
