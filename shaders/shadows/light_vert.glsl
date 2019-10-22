#version 450

layout(location = 0) in vec3 position;

layout(set = 0, binding = 0) uniform Model {
  mat4 model;
} model;

layout(set = 1, binding = 0) uniform lightMatrix {
  mat4 matrix;
} light_matrix;

void main() {
  gl_Position = light_matrix.matrix * model.model * vec4(position, 1.0);
}
