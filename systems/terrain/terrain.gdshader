shader_type spatial;
render_mode world_vertex_coords;

// --- Uniforms ---
uniform sampler2D heightmap : source_color, filter_linear_mipmap, repeat_enable;

// Vertical Scale (Height in meters)
uniform float height_scale = 3000.0;

// Horizontal Map Size (World units covered by 1 repeat of the texture)
uniform float map_size = 200000.0;

// NEW: UV Scale (Tiling factor)
// Increase this if the terrain looks too "stretched" or low-res
uniform float uv_scale = 1.0;

global uniform bool terrain_do_debug_color;

// --- Instance Uniforms ---
instance uniform int lod_level; 
instance uniform vec4 hole_rect; // (MinX, MinZ, MaxX, MaxZ)

// --- Bicubic Math ---
vec4 cubic(float v) {
    vec4 n = vec4(1.0, 2.0, 3.0, 4.0) - v;
    vec4 s = n * n * n;
    float x = s.x;
    float y = s.y - 4.0 * s.x;
    float z = s.z - 4.0 * s.y + 6.0 * s.x;
    float w = 6.0 - x - y - z;
    return vec4(x, y, z, w) * (1.0/6.0);
}

float texture_bicubic(sampler2D sampler, vec2 tex_coords) {
    vec2 tex_size = vec2(textureSize(sampler, 0));
    vec2 inv_tex_size = 1.0 / tex_size;
    tex_coords = tex_coords * tex_size - 0.5;
    vec2 fxy = fract(tex_coords);
    tex_coords -= fxy;
    vec4 xcubic = cubic(fxy.x);
    vec4 ycubic = cubic(fxy.y);
    vec4 c = tex_coords.xxyy + vec2(-0.5, +1.5).xyxy;
    vec4 s = vec4(xcubic.xz + xcubic.yw, ycubic.xz + ycubic.yw);
    vec4 offset = c + vec4(xcubic.yw, ycubic.yw) / s;
    offset *= inv_tex_size.xxyy;
    float sample0 = texture(sampler, offset.xz).r;
    float sample1 = texture(sampler, offset.yz).r;
    float sample2 = texture(sampler, offset.xw).r;
    float sample3 = texture(sampler, offset.yw).r;
    float sx = s.x / (s.x + s.y);
    float sy = s.z / (s.z + s.w);
    return mix(mix(sample3, sample2, sx), mix(sample1, sample0, sx), sy);
}

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

void vertex() {
    // 1. Calculate UVs with the new Scale
    // We center the scaling around (0,0) by multiplying AFTER the divide
    vec2 uv = (VERTEX.xz / map_size) * uv_scale + 0.5;
    
    // 2. Apply Height
    float h = texture_bicubic(heightmap, uv) * height_scale;
    VERTEX.y = h;

    // 3. Recalculate Normals
    // Note: We must apply uv_scale to the step size 'e' as well 
    // to keep the slope calculation accurate relative to the scale!
    float e = (1.0 / map_size) * uv_scale; 

    float h_left  = texture_bicubic(heightmap, uv + vec2(-e, 0)) * height_scale;
    float h_right = texture_bicubic(heightmap, uv + vec2( e, 0)) * height_scale;
    float h_down  = texture_bicubic(heightmap, uv + vec2(0, -e)) * height_scale;
    float h_up    = texture_bicubic(heightmap, uv + vec2(0,  e)) * height_scale;

    vec3 tangent_x = normalize(vec3(2.0, h_right - h_left, 0.0));
    vec3 tangent_z = normalize(vec3(0.0, h_up - h_down, 2.0));
    NORMAL = normalize(cross(tangent_z, tangent_x));
}

void fragment() {
    // Hole Cutting Logic (Dynamic Clipmap)
    if (hole_rect.x != hole_rect.z) {
        vec3 world_pos = (INV_VIEW_MATRIX * vec4(VERTEX, 1.0)).xyz;
        if (world_pos.x > hole_rect.x && world_pos.x < hole_rect.z &&
            world_pos.z > hole_rect.y && world_pos.z < hole_rect.w) {
            discard;
        }
    }

    if (terrain_do_debug_color) {
        float hue = float(lod_level) * 0.125; 
        ALBEDO = hsv2rgb(vec3(hue, 0.8, 0.9)); 
        ROUGHNESS = 1.0;
        METALLIC = 0.0;
    } else {
        ALBEDO = vec3(0.5); 
        ROUGHNESS = 0.8;
    }
}