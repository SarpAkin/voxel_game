#version 450

struct CameraData{
    mat4 proj_view;
};

layout(set = 0,binding = 0) uniform CameraData_{
    CameraData cam;
};

struct Chunk{
    ivec3 pos;
    uint flags;
};



layout(std430,set = 1,binding = 0) readonly buffer ChunkData{
    Chunk chunks[];
};

layout(location = 0) in vec3 vpos;
layout(location = 1) in vec2 v_uv;

layout(location = 0) out vec2 f_uv;


void main() {

    // fcolor = vec3(vcolor & 0xFF,vcolor >> 8 & 0xFF,vcolor >> 16 & 0xFF);
    f_uv = v_uv;

    Chunk chunk = chunks[gl_InstanceIndex];

    gl_Position = cam.proj_view * vec4(vpos + vec3(chunk.pos * 32),1.0);
}

