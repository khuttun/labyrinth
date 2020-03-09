#version 330

uniform vec3 lightPos;
uniform vec3 materialColor;

in vec3 fragPos;
in vec3 fragNormal;

out vec4 outputColor;

vec3 gammaCorrect(vec3 color)
{
    return pow(color, vec3(1.0/2.2));
}

void main()
{
    vec3 normal = normalize(fragNormal);
    vec3 lightDir = normalize(lightPos - fragPos);
    float diffuseCoeff = max(0.0, dot(normal, lightDir));
    
    vec3 ambient = 0.06 * materialColor;
    vec3 diffuse = diffuseCoeff * materialColor;
    vec3 color = ambient + diffuse;
    
    outputColor = vec4(gammaCorrect(color), 1.0);
}