struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(2) tint: vec4<f32>,
    @location(3) params: vec4<f32>,
}

@group(0) @binding(0) var sprite_texture: texture_2d_array<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) layer: u32,
    @location(3) tint: vec4<f32>,
    @location(4) params: vec4<f32>,
) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;
    output.layer = layer;
    output.tint = tint;
    output.params = params;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let flash = clamp(input.params.x, 0.0, 1.0);
    let glitch = clamp(input.params.y, 0.0, 1.0);
    let opacity = clamp(input.params.z, 0.0, 1.0);
    let split = input.params.w;
    let band = floor(input.uv.y * 33.0);
    let tear = sin(band * 12.9898 + glitch * 41.0) * 0.018 * glitch;
    let shifted_uv = vec2<f32>(clamp(input.uv.x + tear, 0.0, 1.0), input.uv.y);
    let center = textureSample(sprite_texture, sprite_sampler, shifted_uv, input.layer);
    let red = textureSample(
        sprite_texture,
        sprite_sampler,
        vec2<f32>(clamp(shifted_uv.x + split, 0.0, 1.0), shifted_uv.y),
        input.layer,
    );
    let blue = textureSample(
        sprite_texture,
        sprite_sampler,
        vec2<f32>(clamp(shifted_uv.x - split * 0.7, 0.0, 1.0), shifted_uv.y),
        input.layer,
    );
    var color = vec3<f32>(red.r, center.g, blue.b);
    color = mix(color, vec3<f32>(0.96, 1.0, 0.98), flash * 0.86);
    color = color * input.tint.rgb;
    let alpha = center.a * input.tint.a * opacity;
    return vec4<f32>(color * alpha, alpha);
}
