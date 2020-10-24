#version 450

layout(location=0) in vec3 position;
layout(location=1) in vec3 normal;
layout(location=2) in vec2 texCoords;

layout(location=0) out vec3 fragPosCamSpace;
layout(location=1) out vec3 fragNormalCamSpace;
layout(location=2) out vec2 fragTexCoords;

layout(set=0, binding=0) 
uniform Uniforms {
    mat4 modelView;
    mat4 normalModelView;
    mat4 projection;
    vec4 lightPosCamSpace;
};

void main()
{
    vec4 p = modelView * vec4(position, 1.0);

    fragPosCamSpace = p.xyz;
    fragNormalCamSpace = normalize(mat3(normalModelView) * normal);
    fragTexCoords = texCoords;

    gl_Position = projection * p;
}
