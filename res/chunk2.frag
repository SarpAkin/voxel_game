#version 450

layout (location = 0) in vec2 f_uv;
layout (location = 1) in vec3 f_normal;
layout (location = 2) in float f_ao;
// layout (location = 3) in vec2 f_debug_uv;

layout (location = 0) out vec4 albedo;
layout (location = 1) out vec4 normal;


layout(set = 2,binding = 0) uniform sampler2D textures[1];

void main()
{
    albedo = vec4(texture(textures[0],f_uv.xy).xyz * max(1.0 - f_ao * f_ao,0.2),1.0);
    normal = vec4(f_normal,0.0);    
    // albedo = vec4(f_ao.xxx,0);
    // if (abs(f_debug_uv.y - 0.5) < 0.02){
    //     albedo = vec4(0,f_debug_uv.x,0,0);
    // }
    // if (abs(f_debug_uv.x - 0.5) < 0.02){
    //     albedo = vec4(0,0,f_debug_uv.y,0);
    // }
}