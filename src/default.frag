#version 330

uniform vec3 lightPosCamSpace;
uniform sampler2D tex;

in vec3 fragPosCamSpace;
in vec3 fragNormalCamSpace;
in vec2 fragTexCoords;

out vec4 outputColor;

void main()
{
    vec3 normal = normalize(fragNormalCamSpace);
    vec3 lightDir = normalize(lightPosCamSpace - fragPosCamSpace);
    float diffuseCoeff = max(0.0, dot(normal, lightDir));

    vec4 materialColor = texture(tex, fragTexCoords);
    vec3 ambient = 0.06 * materialColor.rgb;
    vec3 diffuse = diffuseCoeff * materialColor.rgb;
    vec3 color = ambient + diffuse;

    outputColor = vec4(color, materialColor.a);
}