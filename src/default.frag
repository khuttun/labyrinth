#version 450

layout(location=0) in vec3 fragPosWorldSpace;
layout(location=1) in vec3 fragNormalWorldSpace;
layout(location=2) in vec2 fragTexCoords;

layout(location=0) out vec4 outputColor;

layout(set=0, binding=0) 
uniform SceneUniforms {
    mat4 viewProjection;
    mat4 lightProjection;
    vec4 lightPosWorldSpace;
};

layout(set=1, binding=0) uniform texture2D shadowMap;
layout(set=1, binding=1) uniform samplerShadow shadowMapSampler;

layout(set=3, binding=0) uniform texture2D txtr;
layout(set=3, binding=1) uniform sampler smplr;

// "Shadow coefficient": 0 (completely in shadow) ... 1 (completely in light)
float shadowCoeff(vec4 posLightSpaceProjected)
{
    vec3 posLightSpaceNdc = posLightSpaceProjected.xyz / posLightSpaceProjected.w;

    if (abs(posLightSpaceNdc.x) > 1.0 || abs(posLightSpaceNdc.y) > 1.0 || abs(posLightSpaceNdc.z) > 1.0)
        return 0.0; // completely outside the light's influence

    float darkness = 0.1;
    return 1.0 - darkness + darkness * texture(
        sampler2DShadow(shadowMap, shadowMapSampler),
        vec3(
            // Transform NDC coordinates to texture UV coordinates
            0.5 * posLightSpaceNdc.x + 0.5,
            -0.5 * posLightSpaceNdc.y + 0.5,
            // The comparison value given to the sampler
            posLightSpaceNdc.z
        )
    );
}

void main()
{
    float shadow = shadowCoeff(lightProjection * vec4(fragPosWorldSpace, 1.0));

    // Diffuse component (based on light/surface angle)
    vec3 normal = normalize(fragNormalWorldSpace);
    vec3 lightDir = normalize(lightPosWorldSpace.xyz - fragPosWorldSpace);
    float diffuseCoeff = max(0.0, dot(normal, lightDir));

    // Put all the lighting components together
    vec4 materialColor = texture(sampler2D(txtr, smplr), fragTexCoords);
    vec3 ambient = 0.06 * materialColor.rgb;
    vec3 diffuse = diffuseCoeff * materialColor.rgb;
    vec3 color = ambient + shadow * diffuse;

    outputColor = vec4(color, materialColor.a);
}