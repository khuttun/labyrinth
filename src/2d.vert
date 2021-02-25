#version 450

layout(location=0) in vec2 position;
layout(location=1) in vec2 texCoords;
layout(location=2) in vec4 color;

layout(location=0) out vec2 fragTexCoords;
layout(location=1) out vec4 fragColor;

layout(set=0, binding=0) uniform ObjectUniforms {
    mat4 model;
};

void main()
{
    fragTexCoords = texCoords;
    fragColor = color;
    gl_Position = vec4((mat3(model) * vec3(position, 1.0)).xy, 0.0, 1.0);
}
