#version 330

uniform vec3 lightPosCamSpace;

// Fragment position and normal in camera space
in vec3 fragPos;
in vec3 fragNormal;

out vec4 outputColor;

vec3 gammaCorrect(vec3 color)
{
    return pow(color, vec3(1.0/2.2));
}

// Blinn-Phong shading
void main()
{
    vec3 materialColor = vec3(0.6, 0.3, 0.0);
    float materialShininess = 100.0;
    vec3 specularColor = vec3(1.0, 1.0, 1.0);
    
    vec3 normal = normalize(fragNormal);
    vec3 light = lightPosCamSpace - fragPos;
    vec3 lightDir = normalize(light);
    
    float diffuseCoeff = max(0.0, dot(normal, lightDir));
    
    float specularCoeff = 0.0;
    if (diffuseCoeff > 0.0)
    {
        vec3 camDir = normalize(-fragPos);
        vec3 halfAngle = normalize(lightDir + camDir);
        float reflectedAmount = max(0.0, dot(normal, halfAngle));
        specularCoeff = pow(reflectedAmount, materialShininess);
    }
    
    float lightDist = length(light);
    float attenuation = 1.0 / (1.0 + 0.01 * pow(lightDist, 2.0));
    
    vec3 ambient = 0.06 * materialColor;
    vec3 diffuse = diffuseCoeff * materialColor;
    vec3 specular = specularCoeff * specularColor;
    vec3 color = ambient + attenuation * (diffuse + specular);
    
    outputColor = vec4(gammaCorrect(color), 1.0);
}