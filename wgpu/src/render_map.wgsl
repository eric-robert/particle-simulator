struct VertexOutput {
    @builtin(position) p: vec4<f32>
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

@vertex
fn main_vs(
    @location(0) strength: f32,
    @location(1) position: vec3<f32>,
    @builtin(instance_index) in_instance_index: u32,
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {

    var final_value: VertexOutput;

    let vertex_scale = 1.0;

    var grid_size = consts.mapFidelity;
    let cell_scale = 2.0 / f32(grid_size);

    let cell_x = i32(in_instance_index) % i32(grid_size);
    let cell_y = i32(in_instance_index) / i32(grid_size);

    let cell_center_x = -1.0 + (cell_scale * f32(cell_x)) + cell_scale / 2.0;
    let cell_center_y = -1.0 + (cell_scale * f32(cell_y)) + cell_scale / 2.0;

    let v_x = cell_center_x + (position.x * vertex_scale * cell_scale);
    let v_y = cell_center_y + (position.y * vertex_scale * cell_scale);
        
    final_value.p = vec4<f32>(vec3<f32>(v_x, v_y, strength ), 1.0);

    return final_value;
}

@fragment
fn main_fs(i: VertexOutput) -> @location(0) vec4<f32> {

    var v = i.p.z;
    var c1 = v;
    var c2 = 0.0;
    var c3 = 0.0;

    if (v > 0.33 && v < 0.66) {
        c1 = 0.33;
        c2 = v - 0.33;
    }
    if (v > 0.66) {
        c1 = 0.33;
        c2 = 0.33;
        c3 = v - 0.66;
        c1 = 0.33 - c3;
    }
    return vec4<f32>(c2,c3,c1, 1.0);

}
