struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) edge: f32,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) edge: f32,
) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(position, 0.0, 1.0);
    output.color = color;
    output.edge = edge;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let falloff = max(0.0, 1.0 - input.edge * input.edge);
    let alpha = input.color.a * falloff;
    return vec4<f32>(input.color.rgb * falloff, alpha);
}
