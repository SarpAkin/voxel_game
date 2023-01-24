#version 450

layout(location = 0) in vec3 vpos;
// layout(location = 1) in uint vcolor;

layout(location = 0) out vec3 fcolor;

layout(push_constant) uniform Push{
    mat4 mvp;
};

layout(set = 0,binding = 0) uniform CamBuffer{
    mat4 proj_view;
};


void main() {

    fcolor = vec3(0.0,0.5,1.0);


    // vec4 pos4 = mvp * vec4(vpos,1.0);
    // vec3 pos = pos4.xyz / pos4.w;
    // pos.z = pos.z * 0.5 + 0.5;

    gl_Position = mvp * vec4(vpos,1.0);
}

