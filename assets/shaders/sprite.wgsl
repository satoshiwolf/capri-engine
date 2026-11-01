struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
}

@group(0) @binding(0) var sprite_texture: texture_2d_array<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) layer: u32,
) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;
    output.layer = layer;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sample = textureSample(sprite_texture, sprite_sampler, input.uv, input.layer);
    // The composition target expects premultiplied alpha.
    return vec4<f32>(sample.rgb * sample.a, sample.a);
}
