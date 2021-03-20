#version 450

layout(location=0) in vec3 fragPosWorldSpace;
layout(location=1) in vec3 fragNormalWorldSpace;
layout(location=2) in vec2 fragTexCoords;

layout(location=0) out vec4 outputColor;

layout(set=0, binding=0) uniform SceneUniforms {
    mat4 viewProjection;
    uvec4 numLights;
};

const int MAX_LIGHTS = 4;

struct Light {
    mat4 viewProjection;
    vec4 posWorldSpace;
};

layout(set=0, binding=1) uniform LightUniforms {
    Light lights[MAX_LIGHTS];
};

layout(set=1, binding=0) uniform texture2DArray shadowMaps;
layout(set=1, binding=1) uniform samplerShadow shadowMapSampler;

layout(set=3, binding=0) uniform texture2D objectTexture;
layout(set=3, binding=1) uniform sampler objectTextureSampler;

// "Shadow coefficient": 0 (completely in shadow) ... 1 (completely in light)
float shadowCoeff(int lightId, vec4 posLightSpaceProjected)
{
    vec3 posLightSpaceNdc = posLightSpaceProjected.xyz / posLightSpaceProjected.w;
    const float DARKNESS_COEFF = 0.25;
    return 1.0 - DARKNESS_COEFF + DARKNESS_COEFF * texture(sampler2DArrayShadow(shadowMaps, shadowMapSampler),
        vec4(
            // Transform NDC coordinates to texture UV coordinates
            0.5 * posLightSpaceNdc.x + 0.5,
            -0.5 * posLightSpaceNdc.y + 0.5,
            // Texture array layer
            lightId,
            // The comparison value given to the sampler
            posLightSpaceNdc.z
        )
    );
}

void main()
{
    vec3 normal = normalize(fragNormalWorldSpace);
    
    float luminance = 0.15;
    for (int i = 0; i < numLights.x; ++i) {
        Light light = lights[i];
        vec3 lightDir = normalize(light.posWorldSpace.xyz - fragPosWorldSpace);
        float diffuse = max(0.0, dot(normal, lightDir));
        float shadow = shadowCoeff(i, light.viewProjection * vec4(fragPosWorldSpace, 1.0));
        luminance += shadow * diffuse / numLights.x;
    }

    vec4 materialColor = texture(sampler2D(objectTexture, objectTextureSampler), fragTexCoords);
    outputColor = vec4(luminance * materialColor.rgb, materialColor.a);
}
