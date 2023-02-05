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

layout(std430,set = 3,binding = 0) readonly buffer QuadBuffer{
    uvec2 compressed_quads[];
};

vec3 vpos;
vec2 v_uv;
vec3 vnormal;

//  2-------3
//  |  \    |
//  |   \   |  y+ 
//  |     \ | | uv_coord
//  0-------1 +-> x+  

void init_vert_data(){
    uint quad_id = gl_VertexIndex >> 2; // divide by 4 
    uint vert_index = gl_VertexIndex & 0x3; // get the vertex index in quad [0-3]

    uvec2 data = compressed_quads[quad_id]; 
    uint data_0 = data.x;
    vpos.x = (data_0      ) & 31;
    vpos.y = (data_0 >>  5) & 31;
    vpos.z = (data_0 >> 10) & 31;

    uint direction = (data_0 >> 15) & 7; //from 0-6 x+,x-,y+,y-,z+,z-
    uint material  = (data_0 >> 18);

    vec2 texture_size = vec2(1.0 / 16.0,1);
    v_uv = vec2(float(material) * texture_size.x,0);
    
    uint offset_axies_uvx[6] = {1,2,2,0,0,1};
    uint offset_axies_uvy[6] = {2,1,0,2,1,0};
    
    if ((vert_index & 1) != 0){ // verticies 1,3 uv x+
        v_uv.x += texture_size.x;

        vpos[offset_axies_uvx[direction]] += 1.0;
    }

    if((vert_index >> 1) != 0){ // verticies 2,3 uv y+
        v_uv.y += texture_size.y;

        vpos[offset_axies_uvy[direction]] += 1.0; 
    }

    // if the direction is positive add 1 to the axis of it 
    vpos[direction >> 1] += (1 - (direction & 1));

    vec3 normal_table[6] = {
        vec3( 1.0, 0.0, 0.0),
        vec3(-1.0, 0.0, 0.0),
        vec3( 0.0, 1.0, 0.0),
        vec3( 0.0,-1.0, 0.0),
        vec3( 0.0, 0.0, 1.0),
        vec3( 0.0, 0.0,-1.0),
    };

    vnormal = normal_table[direction];
}


layout(location = 0) out vec2 f_uv;
layout(location = 1) out vec3 f_normal;

void main() {
    init_vert_data();

    f_uv = v_uv;
    f_normal = vnormal;

    Chunk chunk = chunks[gl_InstanceIndex];

    gl_Position = cam.proj_view * vec4(vpos + vec3(chunk.pos * 32),1.0);
}

