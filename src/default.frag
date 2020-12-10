#version 450

layout(location=0) in vec3 fragPosWorldSpace;
layout(location=1) in vec3 fragNormalWorldSpace;
layout(location=2) in vec2 fragTexCoords;

layout(location=0) out vec4 outputColor;

layout(set=0, binding=0) 
uniform SceneUniforms {
    mat4 viewProjection;
    vec4 lightPosWorldSpace;
};

layout(set=2, binding=0) uniform texture2D txtr;
layout(set=2, binding=1) uniform sampler smplr;

void main()
{
    vec3 normal = normalize(fragNormalWorldSpace);
    vec3 lightDir = normalize(lightPosWorldSpace.xyz - fragPosWorldSpace);
    float diffuseCoeff = max(0.0, dot(normal, lightDir));

    vec4 materialColor = texture(sampler2D(txtr, smplr), fragTexCoords);
    vec3 ambient = 0.06 * materialColor.rgb;
    vec3 diffuse = diffuseCoeff * materialColor.rgb;
    vec3 color = ambient + diffuse;

    outputColor = vec4(color, materialColor.a);
}