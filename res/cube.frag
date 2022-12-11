#version 450

layout (location = 0) in vec3 fcolor;

layout (location = 0) out vec4 albedo;
layout (location = 1) out vec4 normal;


void main()
{
    albedo = vec4(fcolor,1.0);
    normal = vec4(0.0,0.0,0.0,0.0);
}