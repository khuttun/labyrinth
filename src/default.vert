#version 330

uniform mat4 modelViewProjection;

in vec3 position;
in vec3 normal;

out vec3 fragPos;
out vec3 fragNormal;

void main()
{
    fragPos = position;
    fragNormal = normal;
    gl_Position = modelViewProjection * vec4(position, 1.0);
}
