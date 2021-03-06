#version 450

layout(location=0) in vec3 position;
layout(location=1) in vec3 normal;
layout(location=2) in vec2 texCoords;

layout(set=0, binding=0) uniform ShadowPassUniforms {
    mat4 viewProjection;
};

layout(set=1, binding=0) uniform ObjectUniforms {
    mat4 model;
    mat4 normalModel;
};

void main()
{
    gl_Position = viewProjection * model * vec4(position, 1.0);
}
