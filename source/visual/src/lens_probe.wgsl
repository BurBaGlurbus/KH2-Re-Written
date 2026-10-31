// The depth under the sun's test block, written out at the block's size so it
// can be copied (a depth texture copies only whole). See lens_probe.rs.

#ifdef MULTISAMPLED
@group(0) @binding(0) var depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(0) var depth: texture_depth_2d;
#endif
@group(0) @binding(1) var<uniform> origin: vec4<u32>;

@fragment
fn fragment(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = vec2<i32>(at.xy) + vec2<i32>(origin.xy);
    return vec4<f32>(textureLoad(depth, texel, 0), 0.0, 0.0, 1.0);
}
