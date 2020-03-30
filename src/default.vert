#version 330

uniform mat4 modelViewProjection;

in vec3 position;
in vec3 normal;
in vec2 tex_coords;

out vec3 fragPos;
out vec3 fragNormal;
out vec2 fragTexCoords;

void main()
{
    fragPos = position;
    fragNormal = normal;
    fragTexCoords = tex_coords;
    gl_Position = modelViewProjection * vec4(position, 1.0);
}
