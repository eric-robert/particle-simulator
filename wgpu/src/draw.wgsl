struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>
};

struct SimulationConstants {
    simulationSpeed : f32,
    mapBounds : f32,
    oobForce : f32,
    maxSpeed : f32,
    minSpeed : f32,
    mapFidelity : f32,
    cellImpact : f32,
    senseDistance : f32,
    senseForce : f32,
    fadePower : f32,
    erasePower : f32,
    instabilityScore : f32,
};

@group(0) @binding(0) var<uniform> consts : SimulationConstants;
//@group(1) @binding(0) var<storage, read_write> indicators : array<f32>;

@vertex
fn main_vs(
    @location(0) particle_pos: vec3<f32>,
    @location(1) particle_vel: vec3<f32>,
    @location(2) position: vec3<f32>,
) -> VertexOutput {


    var output_val: VertexOutput;
    var angle = -atan2(particle_vel.x, particle_vel.y) + 3.1425;

    let pos = vec2<f32>(
        position.x * cos(angle) - position.y * sin(angle),
        position.x * sin(angle) + position.y * cos(angle)
    );

    var tx :f32 = (pos.x) - particle_pos.x;
    var ty :f32 = (pos.y) - particle_pos.y;
    var tz :f32 = position.z - particle_pos.z;

    tz = (tz + 1.0) / 2.0;

    if (tz < 0.0) { tz = 0.01; }
    if (tz > 1.0) { tz = 0.99; }

    output_val.clip_position = vec4<f32>(vec3<f32>(-tx, ty, tz), 1.0);
    return output_val;
}

@fragment
fn main_fs(i: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(i.clip_position.z, i.clip_position.z, i.clip_position.z, 1.0);
}
