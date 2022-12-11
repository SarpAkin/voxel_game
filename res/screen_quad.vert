#version 450

layout (location = 0) out vec2 screen_pos;

void main() 
{
    vec2 uv = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    gl_Position = vec4(uv * 2.0f + -1.0f, 0.0f, 1.0f);
    screen_pos = gl_Position.xy;
}