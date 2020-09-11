#version 330

uniform mat4 modelView;
uniform mat3 normalModelView;
uniform mat4 projection;

in vec3 position;
in vec3 normal;
in vec2 tex_coords;

out vec3 fragPosCamSpace;
out vec3 fragNormalCamSpace;
out vec2 fragTexCoords;

void main()
{
    vec4 p = modelView * vec4(position, 1.0);

    fragPosCamSpace = p.xyz;
    fragNormalCamSpace = normalize(normalModelView * normal);
    fragTexCoords = tex_coords;

    gl_Position = projection * p;
}
