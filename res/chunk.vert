#version 450

layout(location = 0) in vec3 vpos;
layout(location = 1) in vec2 v_uv;

layout(location = 0) out vec2 f_uv;

layout(push_constant) uniform Push{
    mat4 mvp;
};

void main() {

    // fcolor = vec3(vcolor & 0xFF,vcolor >> 8 & 0xFF,vcolor >> 16 & 0xFF);
    f_uv = v_uv;

    gl_Position = mvp * vec4(vpos,1.0);
}

