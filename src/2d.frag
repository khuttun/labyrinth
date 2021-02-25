#version 450

layout(location=0) in vec2 fragTexCoords;
layout(location=1) in vec4 fragColor;

layout(location=0) out vec4 outputColor;

layout(set=1, binding=0) uniform texture2D objectTexture;
layout(set=1, binding=1) uniform sampler objectTextureSampler;

void main()
{
    outputColor = fragColor * texture(sampler2D(objectTexture, objectTextureSampler), fragTexCoords);
}
