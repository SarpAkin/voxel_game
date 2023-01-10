#version 450

layout(location = 0) in vec2 screen_pos;

layout(location = 0) out vec4 color;

layout(set = 0,binding = 0) uniform sampler2D albedo_spec;



void main(){
    color = texture(albedo_spec,screen_pos * .5 + .5);
    // color = vec4(1.0,0.0,0.0,1.0);
}