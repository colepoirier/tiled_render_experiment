#import bevy_pbr::mesh_view_bindings

@group(1) @binding(0)
var our_texture: texture_2d<f32>;

@group(1) @binding(1)
var our_sampler: sampler;

@fragment
fn fragment(
    @builtin(position) position: vec4<f32>,
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {
    // Get screen position with coordinates from 0 to 1
    let uv = position.xy / vec2<f32>(view.width, view.height);

    // var output_color = textureSample(our_texture, our_sampler, uv);

    var output_color = vec4<f32>(0.0, 1.0, 0.0, 1.0);

    return output_color;
}
