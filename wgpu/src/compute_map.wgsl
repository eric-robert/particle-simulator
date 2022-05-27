
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
@group(1) @binding(0) var<storage, read_write> map : array<f32>;
@group(2) @binding(0) var<storage, read_write> indicator : array<f32>;

fn who_cell (x : i32, y : i32) -> i32 {
    
    var _x = x;
    var _y = y;

    if ( _x >= i32(consts.mapFidelity) ) { _x = _x - i32(consts.mapFidelity); }
    if ( _x < 0 ) { _x = _x + i32(consts.mapFidelity); }
    if ( _y < 0 ) { _y = _y + i32(consts.mapFidelity); }
    if ( _y >= i32(consts.mapFidelity) ) { _y = _y - i32(consts.mapFidelity); }

    return  _y * i32(consts.mapFidelity) + _x ;
}

fn read_cell ( x : i32, y : i32) -> f32 {
    return map[ who_cell(x, y) ];
}


@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {

    let index = global_invocation_id.x;
    if (index >= u32(consts.mapFidelity * consts.mapFidelity)) {
        return;
    }

    var grid_size = consts.mapFidelity;
    let cell_x = i32(i32(index) % i32(grid_size));
    let cell_y = i32(i32(index) / i32(grid_size));

    let center = read_cell(cell_x, cell_y);

    map[index] = consts.erasePower * center;
    indicator[index] = 0.0;


    let _left = who_cell(cell_x - 1, cell_y);
    let _right = who_cell(cell_x + 1, cell_y);
    let _top = who_cell(cell_x, cell_y - 1);
    let _bottom = who_cell(cell_x, cell_y + 1);

    let _take_left = map[_left] * consts.fadePower;
    map[_left] = map[_left] - _take_left;

    let _take_right = map[_right] * consts.fadePower;
    map[_right] = map[_right] - _take_right;
    
    let _take_top = map[_top] * consts.fadePower;
    map[_top] = map[_top] - _take_top;
    
    let _take_bottom = map[_bottom] * consts.fadePower;
    map[_bottom] = map[_bottom] - _take_bottom;

    map[index] = map[index] + (_take_left + _take_right + _take_top + _take_bottom) * 0.6;

    if (map[index] > 1.0) {
        map[index] = 1.0;
    }
    if (map[index] < 0.00001) {
        map[index] = 0.0;
    }

}


