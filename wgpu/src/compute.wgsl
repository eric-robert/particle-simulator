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
@group(1) @binding(0) var<storage, read_write> particles : array<f32>;
@group(2) @binding(0) var<storage, read_write> map : array<f32>;
@group(3) @binding(0) var<storage, read_write> indicators : array<f32>;

fn get_cell_index (x : f32, y : f32) -> i32 {

    let world_x = ( x + 1.0) / 2.0 * consts.mapFidelity;
    let world_y = ( -y + 1.0) / 2.0 * consts.mapFidelity;

    var index_x = floor( world_x );
    var index_y = floor( world_y );

    if (index_x < 0.0 ) { index_x = index_x + consts.mapFidelity; }
    if (index_y < 0.0 ) { index_y = index_y + consts.mapFidelity; }
    if (index_y > consts.mapFidelity - 1.0 ) { index_y = index_y - consts.mapFidelity; }
    if (index_x > consts.mapFidelity - 1.0 ) { index_x = index_x - consts.mapFidelity; }
    
    return i32(index_y * consts.mapFidelity + index_x);
}

fn read_cell (x : f32, y : f32) -> f32 {
    return map[ get_cell_index(x, y) ];
}

fn sense_indexes ( x : f32, y : f32, rotation : f32, distance : f32 ) -> vec3<i32> {

    let dx = cos( rotation );
    let dy = sin( rotation );
    
    let x_1 = x + dx * distance;
    let y_1 = y + dy * distance;
    let i_1 = get_cell_index(x_1, y_1);
    indicators[i_1] = 1.0;

    let x_2 = x_1 + dx * distance;
    let y_2 = y_1 + dy * distance;
    let i_2 = get_cell_index(x_2, y_2);
    indicators[i_2] = 1.0;

    let x_3 = x_2 + dx * distance;
    let y_3 = y_2 + dy * distance;
    let i_3 = get_cell_index(x_3, y_3);
    indicators[i_3] = 1.0;

    return vec3<i32>(i_1, i_2, i_2);
}

fn sense_at_angle ( x : f32, y : f32, rotation : f32, distance : f32 ) -> vec3<f32> {

    let indexes = sense_indexes( x, y, rotation, distance );
    let values = vec3<f32>( map[ indexes.x ], map[ indexes.y ], map[ indexes.z ] );

    // Indicate if we are increasing or decreasing
    //let increasing = ( values.z > values.y ) && ( values.y > values.x );
    
    // Indicate average of the three cells
    let average = ( values.x + values.y + values.z ) / 3.0;

    return vec3<f32>( average, values.z - values.x, rotation );

}

fn choose_one ( a : vec3<f32>, b : vec3<f32>, c : vec3<f32>, instability : f32 ) -> vec3<f32> {
    
    let a_value = abs(instability - a.x);
    let b_value = abs(instability - b.x);
    let c_value = abs(instability - c.x);

    if ( a_value < b_value && a_value < c_value ) {
        return a;
    } else if ( b_value < c_value ) {
        return b;
    } else {
        return c;
    }

}

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {

    // Construct particle stuff

    let index = global_invocation_id.x;

    var _1 = u32(1);
    var _2 = u32(2);
    var _3 = u32(3);

    var i_p = index * u32(6);
    var i_v = i_p + _3;

    var _dist = particles[i_p+_2];
    var _target = particles[i_p+_3+_2];

    var pos = vec2(
        particles[i_p],
        particles[i_p+_1]
    );

    var vel = vec2(
        particles[i_v],
        particles[i_v+_1]
    );

    // Guard checking

    if ( pos.x < -1.0 ) {  pos.x = pos.x + 2.0; }
    if ( pos.x > 1.0 ) { pos.x = pos.x - 2.0; }
    if ( pos.y < -1.0 ) { pos.y = pos.y + 2.0; }
    if ( pos.y > 1.0 ) { pos.y = pos.y - 2.0; }


    var speed = sqrt(vel.x * vel.x + vel.y * vel.y);
    if ( speed > consts.maxSpeed) {
        vel.x = vel.x * (consts.maxSpeed / speed);
        vel.y = vel.y * (consts.maxSpeed / speed);
    }
    if ( speed < consts.minSpeed) {
        vel.x = vel.x * (consts.minSpeed / speed);
        vel.y = vel.y * (consts.minSpeed / speed);
    }
    
    // // If oob, end here
    // var in_bounds = pos.x > -1.0 && pos.x < 1.0 && pos.y > -1.0 && pos.y < 1.0;
    // if ( !in_bounds ) {
    //     particles[i_p] = pos.x;
    //     particles[i_p+_1] = pos.y;
    //     particles[i_v] = vel.x;
    //     particles[i_v+_1] = vel.y;
    //     return;
    // }


    // Flow on the grid


    // Get cell data
    let angle = -atan2(vel.x, vel.y) + 3.141592 / 2.0;
    let offset = 3.141592 / 12.0;

    let left = sense_at_angle( pos.x, pos.y, angle + offset, _dist );
    let center = sense_at_angle( pos.x, pos.y, angle, _dist );
    let right = sense_at_angle( pos.x, pos.y, angle - offset, _dist );

    var best_angle = choose_one( left, center, right, _target );

    // // Get new velocities
    let magnitude = sqrt(vel.x * vel.x + vel.y * vel.y);
    let sense_vx = cos(best_angle.z) * magnitude;
    let sense_vy = sin(best_angle.z) * magnitude;

    // Update angle
    let pull = consts.senseForce;
    let inv = 1.0 - pull;
    vel.x = ( inv * vel.x) + (pull * sense_vx);
    vel.y = ( inv * vel.y) + (pull * sense_vy);

    vel.x *= 1.0 + best_angle.y / 10.0;
    vel.y *= 1.0 + best_angle.y / 10.0;
    
    // Movement


    // Do Movement
    pos = pos + vel;

    // Save new position
    particles[i_p] = pos.x;
    particles[i_p+_1] = pos.y;

    // Save new velocity
    particles[i_v] = vel.x;
    particles[i_v+_1] = vel.y;

    // Update the map
    let i = get_cell_index(pos.x, pos.y);
    map[i] += consts.cellImpact;
    if (map[i] >= 1.0) {
        map[i] = 1.0;
    }
        
}
