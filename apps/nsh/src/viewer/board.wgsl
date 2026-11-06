struct Camera {
    matrix: mat4x4<f32>,
    eye_and_srgb: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) model0: vec4<f32>,
    @location(4) model1: vec4<f32>,
    @location(5) model2: vec4<f32>,
    @location(6) model3: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world_position: vec3<f32>,
};

@vertex
fn vertex_main(input: VertexInput) -> VertexOutput {
    let model = mat4x4<f32>(input.model0, input.model1, input.model2, input.model3);
    let world = model * vec4<f32>(input.position, 1.0);
    let x = cross(input.model1.xyz, input.model2.xyz);
    let y = cross(input.model2.xyz, input.model0.xyz);
    let z = cross(input.model0.xyz, input.model1.xyz);
    let normal_matrix = mat3x3<f32>(x, y, z) * (1.0 / dot(input.model0.xyz, x));
    var output: VertexOutput;
    output.clip_position = camera.matrix * world;
    output.normal = normalize(normal_matrix * input.normal.xyz);
    output.color = input.color.xyz;
    output.world_position = world.xyz;
    return output;
}

@fragment
fn fragment_main(input: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var normal = normalize(input.normal);
    if !front {
        normal = -normal;
    }
    let light = normalize(vec3<f32>(-0.4, -0.5, 1.0));
    let diffuse = max(dot(normal, light), 0.0) * 0.65;
    let back_diffuse = max(dot(normal, -light), 0.0) * 0.35;
    let fill = max(dot(normal, normalize(vec3<f32>(0.6, 0.5, 0.2))), 0.0) * 0.18;
    let halfway = normalize(light + normalize(camera.eye_and_srgb.xyz - input.world_position));
    let specular = pow(max(dot(normal, halfway), 0.0), 40.0) * 0.16;
    var color = input.color * (0.38 + diffuse + back_diffuse + fill) + vec3<f32>(specular);
    // Keep the old GL framebuffer colors when a host only supplies an sRGB surface.
    if camera.eye_and_srgb.w > 0.5 {
        color = select(color / 12.92, pow((color + 0.055) / 1.055, vec3<f32>(2.4)), color > vec3<f32>(0.04045));
    }
    return vec4<f32>(color, 1.0);
}
