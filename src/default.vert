#version 450

layout(location=0) in vec3 position;
layout(location=1) in vec3 normal;
layout(location=2) in vec2 texCoords;

layout(location=0) out vec3 fragPosWorldSpace;
layout(location=1) out vec3 fragNormalWorldSpace;
layout(location=2) out vec2 fragTexCoords;

layout(set=0, binding=0) uniform SceneUniforms {
    mat4 viewProjection;
    uvec4 numLights;
};

layout(set=2, binding=0) uniform ObjectUniforms {
    mat4 model;
    mat4 normalModel;
};

void main()
{
    vec4 p = model * vec4(position, 1.0);

    fragPosWorldSpace = p.xyz;
    fragNormalWorldSpace = normalize(mat3(normalModel) * normal);
    fragTexCoords = texCoords;

    gl_Position = viewProjection * p;
}
