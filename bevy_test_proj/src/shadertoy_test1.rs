// https://www.shadertoy.com/view/7cfGzn

mod rendering;
use rendering::dsl::*;
use shader_macros::ShaderStruct;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wgpu::util::DeviceExt;
use std::sync::Arc;
use bytemuck::{Pod, Zeroable};


#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct ShaderUniforms {
    resolution: [f32; 2],
    time: f32,
    _padding: f32, 
}

#[derive(ShaderStruct, Clone)]
pub struct UniformData {
    pub resolution: Vec2<f32>,
    pub time: f32,
}
struct UniformDataVar;

async fn run() {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().with_title("DSL Shadertoy").build(&event_loop).unwrap());

    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }).await.unwrap();

    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default(), None).await.unwrap();
    
    let size = window.inner_size();
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps.formats[0];

    let mut config = surface.get_default_config(&adapter, window.inner_size().width, window.inner_size().height).unwrap();
    // --- YOUR SHADER DSL TRANSLATION ---

    let palette = define_fn("palette", |i: Var<f32>| mdo! {
        a <- make_float3!(0.50f32, 0.38f32, 0.26f32);
        b <- make_float3!(0.50f32, 0.35f32, 0.25f32);
        c <- make_float3!(1.00f32, 1.00f32, 1.00f32);
        d <- make_float3!(0.00f32, 0.12f32, 0.25f32);
        a + b * cos(6.2831853f32 * (c.in_context() * i + d))
    });
    let palette = &palette;

    // The code-golf rotation matrix unwrapped into safe math
    let rot2 = define_fn("rot2", |p: Var<Vec2<f32>>, a: Var<f32>| mdo! {
        c1 <- cos(a);
        c2 <- cos(a.in_context() + 33.0f32);
        c3 <- cos(a.in_context() + 11.0f32);
        c4 <- cos(a);
        v <- make_float2!(
            p.x().in_context() * c1 + p.y().in_context() * c3,
            p.x().in_context() * c2 + p.y().in_context() * c4
        );
        Free::Pure(v)
    });
    let rot2 = &rot2;

    let final_wgsl = ShaderCode::new()
        .uniform::<UniformDataVar, UniformData>(0, 0)
        .build_pipeline(|builder, globals| {
            
            // FULLSCREEN VERTEX SHADER
            builder.vert("vs_main", |BuiltIn(vi, _): BuiltIn<VertexIndex, u32>| mdo! {
                x <- ((vi.in_context() << 1u32) & 2u32).cast::<f32>();
                y <- (vi.in_context() & 2u32).cast::<f32>();
                v <- make_float4!(x.in_context() * 2.0f32 - 1.0f32, 1.0f32 - y.in_context() * 2.0f32, 0.0f32, 1.0f32);
                Free::Pure(BuiltIn::<Position, _>::new(v))
            })

            // FRAGMENT SHADER (THE SHADERTOY)
            .frag("fs_main", |BuiltIn(pos, _): BuiltIn<Position, Vec4<f32>>| mdo! {
                let uni = globals.get(UniformDataVar);
                
                i_res <- uni.resolution().in_context();
                i_time <- uni.time().in_context();
                
                u <- make_float2!(pos.x(), i_res.y().in_context() - pos.y());
                
                uv <- (u - 0.5f32 * i_res.in_context() + 0.5f32) / i_res.y().in_context();
                
                _div <- floor(i_time.in_context() / 6.283185f32);
                t <- i_time - 6.283185f32 * _div.in_context();
                
                _dx <- 2.0f32 * u.x().in_context() - i_res.x();
                _dy <- 2.0f32 * u.y().in_context() - i_res.y();
                d <- normalize(make_float3!(_dx, _dy, i_res.y()));
                
                p <- make_float3!(0.0f32, 0.0f32, t.clone());
                frag_color <- make_float4!(0.0f32);
                
                // THE RAYMARCH LOOP
                (0..20).for_(move |_idx, (break_, _)| mdo! {
                    i <- _idx.cast::<f32>().in_context();
                    
                    _angle <- -p.z().in_context() * 0.01f32 - t.in_context() * 0.05f32;
                    _rot_xy <- rot2.call(p.xy(), _angle);
                    set(p, make_float3!(_rot_xy.x(), _rot_xy.y(), p.z()));
                    
                    s <- Var::make_f32(0.6f32);
                    
                    _cyl <- 4.0f32 * (-length(p.xy()) + 10.0f32);
                    set(s, max(s.in_context(), _cyl));
                    
                    _wave <- sin(t - p.x().in_context() * 0.5f32) * 0.9f32;
                    _flow <- abs(p.y().in_context() * 0.004f32 + _wave + 1.0f32);
                    set(s, s.clone().in_context() + _flow);
                    
                    set(p, p.clone().in_context() + d.clone() * s.clone().in_context());
                    
                    _glow <- 1.0f32 / (s.in_context() * 0.2f32);
                    _glow4 <- make_float4!(_glow.clone(), _glow.clone(), _glow.clone(), _glow);
                    
                    set(frag_color, frag_color.clone().in_context() + _glow4);
                });
                
                // POST PROCESSING
                _depth <- length(p) / (abs(sin(i_time.in_context() * 0.02f32) * 50.0f32) + 6.0f32);
                _pal <- palette.call(_depth);
                _pal4 <- make_float4!(_pal.x(), _pal.y(), _pal.z(), 1.0f32);
                set(frag_color, frag_color.in_context() * _pal4);
                
                _uv_sin <- sin(uv.in_context() * 200.0f32) / 1.5f32;
                _ss <- smoothstep(
                    0.001f32.in_context(), 
                    abs(sin(i_time.in_context() * 5.0f32)), 
                    0.7f32 - length(_uv_sin) - abs(uv.y()) + 0.2f32   
                );
                _ss4 <- make_float4!(_ss, _ss, _ss, _ss);
                set(frag_color, frag_color.in_context() - _ss4.in_context() * 20.0f32);
                
                set(frag_color, frag_color.in_context() / 50.0f32);
                
                l <- length(uv);
                set(frag_color, frag_color.in_context() * (1.2f32 - l.in_context()));
                
                _pal_center <- palette.call(l.in_context() - 0.23f32);
                // Swizzle .rgbr
                _pc_rgbr <- make_float4!(_pal_center.x(), _pal_center.y(), _pal_center.z(), _pal_center.x());
                
                _mix_factor <- 1.0f32 - smoothstep(0.01f32.in_context(), 0.95f32.in_context(), l);
                set(frag_color, mix(frag_color.in_context(), _pc_rgbr, make_float4!(_mix_factor)));
                
                // WGSL does not have tanh(). tanh(x) = (exp(x) - exp(-x)) / (exp(x) + exp(-x))
                _fc2 <- frag_color.in_context() + frag_color.in_context();
                _exp_p <- exp(_fc2.in_context());
                _exp_n <- exp(-_fc2.in_context());
                set(frag_color, (_exp_p.clone().in_context() - _exp_n.clone().in_context()) / (_exp_p.in_context() + _exp_n));
                
                Free::Pure(Location::<0, _>::new(frag_color))
            })
        });

    // --- WGPU BOILERPLATE ---

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shadertoy"),
        source: wgpu::ShaderSource::Wgsl(final_wgsl.into()),
    });

    let initial_uniforms = ShaderUniforms {
        resolution: [size.width as f32, size.height as f32],
        time: 0.0,
        _padding: 0.0,
    };

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[initial_uniforms]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
        label: Some("bind_group_layout"),
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
        label: Some("bind_group"),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let start_time = std::time::Instant::now();

    let _ = event_loop.run(move |event, elwt| {
        // Set control flow via the EventLoopWindowTarget (elwt)
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            // Use elwt.exit() instead of setting a mutable reference
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
            
            Event::WindowEvent { event: WindowEvent::Resized(physical_size), .. } => {
                config.width = physical_size.width;
                config.height = physical_size.height;
                surface.configure(&device, &config);
            }
            
            // MainEventsCleared is now AboutToWait
            Event::AboutToWait => window.request_redraw(),
            
            // RedrawRequested is now inside WindowEvent
            Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                let time = start_time.elapsed().as_secs_f32();
                let uniforms = ShaderUniforms {
                    resolution: [config.width as f32, config.height as f32],
                    time,
                    _padding: 0.0,
                };
                queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

                let frame = surface.get_current_texture().unwrap();
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Encoder") });

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                // Note: if you recently updated wgpu too, this might need 
                                // to be changed to `store: wgpu::StoreOp::Store`
                                store: wgpu::StoreOp::Store, 
                            },
                        })],
                        occlusion_query_set: None,
                        timestamp_writes: None,
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.draw(0..3, 0..1); // 3 vertices generated in the vertex shader!
                }

                queue.submit(std::iter::once(encoder.finish()));
                frame.present();
            }
            _ => {}
        }
    });
}

fn main() {
    pollster::block_on(run());
}