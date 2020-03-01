#version 330

uniform mat4 modelView;
uniform mat3 normalModelView;
uniform mat4 projection;

in vec3 position;
in vec3 normal;

out vec3 fragPos;
out vec3 fragNormal;

void main()
{
    // Vertex position in camera space
    vec4 v = modelView * vec4(position, 1.0);
    
    // Pass vertex position and normal in camera space to fragmet shader
    fragPos = v.xyz;
    fragNormal = normalize(normalModelView * normal);
    
    gl_Position = projection * v;
}
