#version 450

layout (location = 0) in vec2 f_uv;

layout (location = 0) out vec4 albedo;
layout (location = 1) out vec4 normal;


layout(set = 0,binding = 0) uniform sampler2D texture_0;

void main()
{
    albedo = vec4(texture(texture_0,f_uv.xy).xyz,1.0);
    normal = vec4(0.0,0.0,0.0,0.0);    
}