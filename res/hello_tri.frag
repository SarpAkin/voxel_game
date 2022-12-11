#version 450

layout(set = 0, binding = 0) uniform CameraUBO {
    vec4 color;
}cam;

layout (location = 0) out vec4 albedo;

void main()
{
    albedo = cam.color;
}