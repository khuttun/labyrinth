#version 330

uniform vec3 lightPos;
uniform sampler2D tex;

in vec3 fragPos;
in vec3 fragNormal;
in vec2 fragTexCoords;

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
    
    vec4 materialColor = texture(tex, fragTexCoords);
    vec3 ambient = 0.06 * materialColor.rgb;
    vec3 diffuse = diffuseCoeff * materialColor.rgb;
    vec3 color = ambient + diffuse;
    
    outputColor = vec4(gammaCorrect(color), materialColor.a);
}