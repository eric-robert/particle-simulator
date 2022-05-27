use rand::{ distributions::{Distribution, Uniform}, SeedableRng, Rng};
use std::{borrow::Cow, mem};
use wgpu::util::DeviceExt;
use std::convert::TryFrom;

#[path = "./framework.rs"]
mod framework;

// -------------------------------------------------------------------------------------------------
// Handling control of the application
// -------------------------------------------------------------------------------------------------


// Particle Count
const NUM_PARTICLES: u32 = 3000000;
const WORKGROUPS: u32 = 1 + NUM_PARTICLES / 64;

// How many regions n x n we want to split the screen into
const MAP_FIDELITY: f32 = 500.0;
const MAP_WORKGROUPS: u32 = (1.0 + ((MAP_FIDELITY * MAP_FIDELITY) / 64.0)) as u32;
 
// Used to calculate step sizes
const SIMULATION_SPEED: f32 = 1.0;
const SIMULATION_ITTERATIONS: u32 = 1;

// Where "OOB" starts and how hard to push them back in
// DEAD
const MAP_BOUNDERY: f32 = 0.85;
// DEAD
const OOB_FORCE: f32 = 0.003;

// Max speed moved in 1.0 time step
const MAX_SPEED: f32 = 0.01;
const MIN_SPEED: f32 = 0.0002;

// How much the map changes
const CELL_IMPACT: f32 = 0.001;

// How powerful is the senses
// DEAD
const SENSE_DISTANCE: f32 = 0.04;
const SENSE_FORCE: f32 = 0.4;

// How powerful the fade is
const FADE_POWER: f32 = 0.009;
const ERASE_POWER: f32 = 0.99;

// How stable it should be 1 is perfect
// DEAD
const INSTABLITY: f32 = 0.0;

// Define a single state the represents the application
struct State {

    const_bind_compute_group: wgpu::BindGroup,
    const_bind_vertex_group: wgpu::BindGroup,

    raw_particle_buffer: wgpu::Buffer,
    particle_bind_group: wgpu::BindGroup,

    raw_indicator_buffer: wgpu::Buffer,
    indicator_bind_group_compute: wgpu::BindGroup,

    raw_map_buffer : wgpu::Buffer,
    map_bind_group: wgpu::BindGroup,

    triangle_vertex_buffer: wgpu::Buffer,
    square_vertex_buffer: wgpu::Buffer,

    compute_map_pipeline: wgpu::ComputePipeline,
    compute_pipeline: wgpu::ComputePipeline,
    pipeline_render_particles: wgpu::RenderPipeline,
    pipeline_render_map: wgpu::RenderPipeline,
    pipeline_render_indicators: wgpu::RenderPipeline,

    frame_num: u32,

}

// -------------------------------------------------------------------------------------------------
// Utility functions
// -------------------------------------------------------------------------------------------------

fn make_buffer ( device : &wgpu::Device, source : &Vec<f32> ) -> wgpu::Buffer {
    return device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(source),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::UNIFORM,
    });
}

fn make_shader ( device : &wgpu::Device, source : &str ) -> wgpu::ShaderModule {
    return device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(source))
    });
}

fn make_binding_layout (device : &wgpu::Device, stage : wgpu::ShaderStages, bind_type : wgpu::BufferBindingType, sizes : &[u32]) -> wgpu::BindGroupLayout {

    // Construct the entieries of the bind group
    let mut enties = Vec::<wgpu::BindGroupLayoutEntry>::new();
    for i in 0..sizes.len() {
        enties.push(wgpu::BindGroupLayoutEntry {
            count: None,
            binding: i as u32,
            visibility: stage,
            ty: wgpu::BindingType::Buffer {
                ty: bind_type,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(sizes[i] as _ ),
            }
        });
    }

    // Create the layout
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: enties.as_slice(),
        label: None,
    });

    return layout;

}

fn make_pipeline_layout (device : &wgpu::Device, layouts : &[&wgpu::BindGroupLayout]) -> wgpu::PipelineLayout {

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: layouts,
        push_constant_ranges: &[],
    });

    return layout;
}

fn make_compute_pipeline (device : &wgpu::Device, layout : &wgpu::PipelineLayout, shader : &wgpu::ShaderModule) -> wgpu::ComputePipeline {

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&layout),
        module: &shader,
        entry_point: "main",
    });

    return pipeline;
}

fn make_render_pipeline (device : &wgpu::Device, config: &wgpu::SurfaceConfiguration, layout : &wgpu::PipelineLayout, shader : &wgpu::ShaderModule, buffers : &[wgpu::VertexBufferLayout]) -> wgpu::RenderPipeline {

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "main_vs",
            buffers: buffers
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "main_fs",
            targets: &[config.format.into()],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    return pipeline;
}

fn make_bind_group (device : &wgpu::Device, layout : &wgpu::BindGroupLayout, buffers : &[&wgpu::Buffer]) -> wgpu::BindGroup {

    let mut entries = Vec::<wgpu::BindGroupEntry>::new();
    for i in 0..buffers.len() {
        entries.push(wgpu::BindGroupEntry {
            binding: i as u32,
            resource: buffers[i].as_entire_binding()
        });
    }

    return device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: layout,
        entries: &entries,
        label: None,
    });

}

// -------------------------------------------------------------------------------------------------
// Implement the window and state management
// -------------------------------------------------------------------------------------------------

impl framework::Example for State {
    fn required_limits() -> wgpu::Limits {
        wgpu::Limits::downlevel_defaults()
    }

    fn required_downlevel_capabilities() -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: wgpu::DownlevelFlags::COMPUTE_SHADERS,
            ..Default::default()
        }
    }

    /// constructs initial instance of State struct
    fn init( config: &wgpu::SurfaceConfiguration, _adapter: &wgpu::Adapter, device: &wgpu::Device, _queue: &wgpu::Queue) -> Self {
        

        // Create the shaders
        let compute_particles_shader = make_shader(device, include_str!("compute.wgsl"));
        let compute_map_shader = make_shader(device, include_str!("compute_map.wgsl"));
        let render_particle_shader = make_shader(device, include_str!("draw.wgsl"));
        let render_map_shader = make_shader(device, include_str!("render_map.wgsl"));
        let render_indicator_shader = make_shader(device, include_str!("render_indicators.wgsl"));


        // Construct constants to be bound to shaders
        let constant_data = [
            SIMULATION_SPEED,
            MAP_BOUNDERY,
            OOB_FORCE,
            MAX_SPEED,
            MIN_SPEED,
            MAP_FIDELITY,
            CELL_IMPACT,
            SENSE_DISTANCE,
            SENSE_FORCE,
            FADE_POWER,
            ERASE_POWER,
            INSTABLITY
        ];
        let constant_data_buffer = make_buffer(device, &constant_data.to_vec());

        let triangle_vertex_data = [
            0.0f32  ,   0.01    ,   0.0,
            0.005   ,   -0.005  ,   0.0,
            -0.005  ,   -0.005  ,   0.0];
        let triangle_vertex_buffer = make_buffer(device, &triangle_vertex_data.to_vec());

        let square_vertex_data = [
            -0.5f32 ,   0.5     ,   0.0,
            0.5     ,   0.5     ,   0.0,
            0.5     ,   -0.5    ,   0.0,
            -0.5    ,   0.5     ,   0.0,
            0.5     ,   -0.5    ,   0.0,
            -0.5    ,   -0.5    ,   0.0];
        let square_vertex_buffer = make_buffer(device, &square_vertex_data.to_vec());

        // Setup Bind Layouts

        let _f = mem::size_of::<f32>() as u32;

        let _size = (_f as u32) * (constant_data.len() as u32);
        let _bind_type = wgpu::BufferBindingType::Uniform;
        let binding_constants_compute = make_binding_layout(device, wgpu::ShaderStages::COMPUTE, _bind_type, &[ _size ]);
        let binding_constants_vertex = make_binding_layout(device, wgpu::ShaderStages::VERTEX, _bind_type, &[ _size ]);

        let _size = _f * 6 * usize::try_from(NUM_PARTICLES).unwrap() as u32;
        let _bind_type = wgpu::BufferBindingType::Storage { read_only: false };
        let binding_particles_compute = make_binding_layout(device, wgpu::ShaderStages::COMPUTE, _bind_type, &[ _size ]);

        let _size = _f * ((MAP_FIDELITY * MAP_FIDELITY) as u32);
        let _bind_type = wgpu::BufferBindingType::Storage { read_only: false };
        let _bind_type_2 = wgpu::BufferBindingType::Uniform;
        let binding_map_compute = make_binding_layout(device, wgpu::ShaderStages::COMPUTE, _bind_type, &[ _size ]);
        let binding_map_vertex = make_binding_layout(device, wgpu::ShaderStages::VERTEX, _bind_type_2, &[ _size ]);


        // Create the pipeline layouts

        let pipeline_layout_compute = make_pipeline_layout(device, &[ 
            &binding_constants_compute, 
            &binding_particles_compute, 
            &binding_map_compute,
            &binding_map_compute,
        ]);

        let pipeline_layout_compute_map = make_pipeline_layout(device, &[ 
            &binding_constants_compute, 
            &binding_map_compute,
            &binding_map_compute
        ]);

        let pipeline_layout_render_map = make_pipeline_layout(device, &[
            &binding_constants_vertex
        ]);

        let pipeline_layout_render_indicators = make_pipeline_layout(device, &[
            &binding_constants_vertex
        ]);

        let pipeline_layout_render_particles = make_pipeline_layout(device, &[
            &binding_constants_vertex
        ]);


        // create pipelines with empty bind group layout

        let compute_pipeline = make_compute_pipeline(device, &pipeline_layout_compute, &compute_particles_shader);
        let compute_map_pipeline = make_compute_pipeline(device, &pipeline_layout_compute_map, &compute_map_shader);

        let pipeline_render_particles = make_render_pipeline(device, config, &pipeline_layout_render_particles, &render_particle_shader, &[
            wgpu::VertexBufferLayout {
                array_stride: 6 * 4,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
            },
            wgpu::VertexBufferLayout {
                array_stride: 3 * 4,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![2 => Float32x3],
            },
        ]);


        let pipeline_render_map = make_render_pipeline(device, config, &pipeline_layout_render_map, &render_map_shader, &[
            wgpu::VertexBufferLayout {
                array_stride: 1 * 4,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32],
            },
            wgpu::VertexBufferLayout {
                array_stride: 3 * 4,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![1 => Float32x3],
            },
        ]);

        
        let pipeline_render_indicators = make_render_pipeline(device, config, &pipeline_layout_render_indicators, &render_indicator_shader, &[
            wgpu::VertexBufferLayout {
                array_stride: 1 * 4,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32],
            },
            wgpu::VertexBufferLayout {
                array_stride: 3 * 4,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![1 => Float32x3],
            },
        ]);

        // Actually create the buffers and bindings data


        // Particles

        let mut raw_particle_data = vec![ 0.0f32; (NUM_PARTICLES * 6) as usize];
        let mut rng = rand::rngs::StdRng::seed_from_u64(43);
        let unif = Uniform::new_inclusive(-1.0f32, 1.0);
        for chunk in raw_particle_data.chunks_mut(6) {
            while true {
                let mut x = rng.sample(&unif);
                let mut y = rng.sample(&unif);
                if x * x + y * y >= 1.0f32 { }
                else {
                    x = x * 0.5f32;
                    y = y * 0.5f32;
                    chunk[0] =  x; // posx
                    chunk[1] =  y; // posy
                    chunk[2] =  0.04; // look distance            
                    chunk[3] =  -x * 0.01 + rng.sample(&unif) * 0.01; // velx
                    chunk[4] =  -y * 0.01 + rng.sample(&unif) * 0.01; // vely

                    let mut v = (rng.sample(&unif)+1.0)/2.0;
                    if v < 0.5 { v = 0.0;  }
                    else { v = 1.0; }
                    
                    chunk[5] =  v; // density preference
                    //chunk[5] = chunk[5] * chunk[5];
                    break;
                }
            }
        }

        let raw_particle_buffer = make_buffer(device, &raw_particle_data);
        let particle_bind_group = make_bind_group(device, &binding_particles_compute, &[&raw_particle_buffer]);
        

        // Map Data

        let mut raw_map_data = vec![0.0f32; ((MAP_FIDELITY * MAP_FIDELITY)) as usize];
        let raw_map_buffer = make_buffer(device, &raw_map_data);
        let map_bind_group = make_bind_group(device, &binding_map_compute, &[&raw_map_buffer]);

        let mut raw_indicator_map_data = vec![0.0f32; ((MAP_FIDELITY * MAP_FIDELITY)) as usize];
        let raw_indicator_buffer = make_buffer(device, &raw_indicator_map_data);
        let indicator_bind_group_compute = make_bind_group(device, &binding_map_compute, &[&raw_indicator_buffer]);


        // Constants Data

        let const_bind_compute_group = make_bind_group(device, &binding_constants_compute, &[&constant_data_buffer]);
        let const_bind_vertex_group = make_bind_group(device, &binding_constants_vertex, &[&constant_data_buffer]);


        State {

            const_bind_compute_group,
            const_bind_vertex_group,

            raw_particle_buffer,
            particle_bind_group,

            raw_indicator_buffer,
            indicator_bind_group_compute,

            raw_map_buffer,
            map_bind_group,
            
            triangle_vertex_buffer,
            square_vertex_buffer,

            compute_map_pipeline,
            compute_pipeline,
            pipeline_render_particles,
            pipeline_render_map,
            pipeline_render_indicators,

            frame_num : 0
        }
    }

    /// update is called for any WindowEvent not handled by the framework
    fn update(&mut self, _event: winit::event::WindowEvent) {
        //empty
    }

    /// resize is called on WindowEvent::Resized events
    fn resize(
        &mut self,
        _sc_desc: &wgpu::SurfaceConfiguration,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
        //empty
    }

    /// render is called each frame, dispatching compute groups proportional
    ///   a TriangleList draw call for all NUM_PARTICLES at 3 vertices each
    fn render(
        &mut self,
        view: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _spawner: &framework::Spawner,
    ) {

        // create render pass descriptor and its color attachments
        let color_attachments = [wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            },
        }];
        let render_pass_descriptor = wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
        };

        // get command encoder
        let mut command_encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        for _ in 0..SIMULATION_ITTERATIONS {
            command_encoder.push_debug_group("compute map changes");
            {
                // compute pass
                let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
                cpass.set_pipeline(&self.compute_map_pipeline);
                cpass.set_bind_group(0, &self.const_bind_compute_group, &[]);
                cpass.set_bind_group(1, &self.map_bind_group, &[]);
                cpass.set_bind_group(2, &self.indicator_bind_group_compute, &[]);
                cpass.dispatch(MAP_WORKGROUPS, 1, 1);
            }
            command_encoder.pop_debug_group();

            command_encoder.push_debug_group("compute boid movement");
            {
                // compute pass
                let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
                cpass.set_pipeline(&self.compute_pipeline);
                cpass.set_bind_group(0, &self.const_bind_compute_group, &[]);
                cpass.set_bind_group(1, &self.particle_bind_group, &[]);
                cpass.set_bind_group(2, &self.map_bind_group, &[]);
                cpass.set_bind_group(3, &self.indicator_bind_group_compute, &[]);
                cpass.dispatch(WORKGROUPS, 1, 1);
            }
            command_encoder.pop_debug_group();

            
        }

        command_encoder.push_debug_group("render map");
        {
            // render pass map
            let mut rpass = command_encoder.begin_render_pass(&render_pass_descriptor);
            rpass.set_pipeline(&self.pipeline_render_map);
            rpass.set_bind_group(0, &self.const_bind_vertex_group, &[]);
            rpass.set_vertex_buffer(0, self.raw_map_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.square_vertex_buffer.slice(..));
            rpass.draw(0..6, 0..((MAP_FIDELITY * MAP_FIDELITY) as u32));
        }
        command_encoder.pop_debug_group();

        // command_encoder.push_debug_group("render boids");
        // {
        //     // render pass boids
        //     let mut rpass = command_encoder.begin_render_pass(&render_pass_descriptor);
        //     rpass.set_pipeline(&self.pipeline_render_particles);
        //     rpass.set_bind_group(0, &self.const_bind_vertex_group, &[]);
        //     rpass.set_vertex_buffer(0, self.raw_particle_buffer.slice(..));
        //     rpass.set_vertex_buffer(1, self.triangle_vertex_buffer.slice(..));
        //     rpass.draw(0..3, 0..NUM_PARTICLES);
        // }
        // command_encoder.pop_debug_group();

        // command_encoder.push_debug_group("render indicators");
        // {
        //     // render pass indicators
        //     let mut rpass = command_encoder.begin_render_pass(&render_pass_descriptor);
        //     rpass.set_pipeline(&self.pipeline_render_indicators);
        //     rpass.set_bind_group(0, &self.const_bind_vertex_group, &[]);
        //     rpass.set_vertex_buffer(0, self.raw_indicator_buffer.slice(..));
        //     rpass.set_vertex_buffer(1, self.square_vertex_buffer.slice(..));
        //     rpass.draw(0..6, 0..((MAP_FIDELITY * MAP_FIDELITY) as u32));
        // }
        // command_encoder.pop_debug_group();

        // update frame count
        self.frame_num += 1;

        // done
        queue.submit(Some(command_encoder.finish()));
    }
}

/// run State
fn main() {
    framework::run::<State>("PARTICLES");
}
