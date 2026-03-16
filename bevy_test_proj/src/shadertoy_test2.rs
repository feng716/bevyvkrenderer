// https://www.shadertoy.com/view/Ms2SD1
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
    _padding: f32, // 16-byte alignment
}

#[derive(ShaderStruct, Clone)]
pub struct UniformData {
    pub resolution: Vec2<f32>,
    pub time: f32,
}
struct UniformDataVar;

async fn run() {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().with_title("DSL Seascape").build(&event_loop).unwrap());

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
    surface.configure(&device, &config);

    let hash = define_fn("hash", |p_in: Var<Vec2<f32>>| mdo! {
        // p = p_in - floor(p_in / 289.0) * 289.0;
        p <- p_in.in_context() - floor(p_in.in_context() / 289.0f32) * 289.0f32;
        
        h <- dot(p, make_float2!(127.1f32, 311.7f32));
        fract(sin(h) * 43758.5453123f32.in_context())
    });
    let hash = &hash;

    let noise = define_fn("noise", |p: Var<Vec2<f32>>| mdo! {
        i <- floor(p);
        f <- fract(p);
        u <- f.in_context() * f * (3.0f32.in_context() - 2.0f32.in_context() * f);
        
        h00 <- hash.call(i + make_float2!(0.0f32, 0.0f32));
        h10 <- hash.call(i + make_float2!(1.0f32, 0.0f32));
        h01 <- hash.call(i + make_float2!(0.0f32, 1.0f32));
        h11 <- hash.call(i + make_float2!(1.0f32, 1.0f32));
        
        m1 <- mix(h00, h10, u.x());
        m2 <- mix(h01, h11, u.x());
        
        -1.0f32 + 2.0f32 * mix(m1, m2, u.y())
    });
    let noise = &noise;

    let sea_octave = define_fn("sea_octave", |uv_in: Var<Vec2<f32>>, choppy: Var<f32>| mdo! {
        uv <- uv_in.in_context() + make_float2!(noise.call(uv_in));
        wv <- 1.0f32.in_context() - abs(sin(uv));
        swv <- abs(cos(uv));
        wv_mix <- mix(wv, swv, wv);
        pow(1.0f32 - pow(wv_mix.x().in_context() * wv_mix.y(), 0.65f32), choppy)
    });
    let sea_octave = &sea_octave;

    let build_map = |name: &'static str, iters: i32| {
        define_fn(name, |p: Var<Vec3<f32>>, sea_time: Var<f32>| mdo! {
            freq <- Var::make_f32(0.16f32); // SEA_FREQ
            amp <- Var::make_f32(0.6f32);   // SEA_HEIGHT
            choppy <- Var::make_f32(4.0f32); // SEA_CHOPPY
            
            uv <- make_float2!(p.x().in_context() * 0.75f32, p.z());
            h <- Var::make_f32(0.0f32);
            
            (0..iters).for_(move |_i, _| mdo! {
                d1 <- sea_octave.call((uv.in_context() + sea_time) * freq, choppy);
                d2 <- sea_octave.call((uv.in_context() - sea_time) * freq, choppy);
                set(h, h + (d1.in_context() + d2) * amp);
                
                // octave_m = mat2(1.6, 1.2, -1.2, 1.6)
                uv_new <- make_float2!(
                    uv.x().in_context() * 1.6f32 + uv.y().in_context() * 1.2f32,
                    uv.x().in_context() * -1.2f32 + uv.y().in_context() * 1.6f32
                );
                
                set(uv, uv_new);
                set(freq, freq.in_context() * 1.9f32);
                set(amp, amp.in_context() * 0.22f32);
                set(choppy, mix(choppy, 1.0f32.in_context(), 0.2f32.in_context()));
            });
            p.y().in_context() - h
        })
    };
    
    let map = build_map("map", 3);
    let map = &map;
    let map_detailed = build_map("map_detailed", 5);
    let map_detailed = &map_detailed;

    let get_sky_color = define_fn("get_sky_color", |e_in: Var<Vec3<f32>>| mdo! {
        ey <- (max(e_in.y(), 0.0f32.in_context()) * 0.8f32 + 0.2f32) * 0.8f32;
        inv_y <- 1.0f32.in_context() - ey;
        c <- make_float3!(pow(inv_y, 2.0f32.in_context()), inv_y, 0.6f32 + inv_y.in_context() * 0.4f32);
        c.in_context() * 1.1f32
    });
    let get_sky_color = &get_sky_color;

    let get_sea_color = define_fn("get_sea_color", |p: Var<Vec3<f32>>, n: Var<Vec3<f32>>, l: Var<Vec3<f32>>, eye: Var<Vec3<f32>>, dist: Var<Vec3<f32>>| mdo! {
        _f <- clamp(1.0f32.in_context() - dot(n, -eye.in_context()), 0.0f32, 1.0f32);
        fresnel <- min(_f.in_context() * _f * _f, 0.5f32.in_context());
        
        reflected <- get_sky_color.call(reflect(eye, n));
        
        _diff <- pow(dot(n, l) * 0.4f32 + 0.6f32, 80.0f32.in_context());
        refracted <- make_float3!(0.0f32, 0.09f32, 0.18f32) + _diff * make_float3!(0.48f32, 0.54f32, 0.36f32) * 0.12f32;
        
        color <- make_float3!(mix(refracted, reflected, make_float3!(fresnel)));
        
        atten <- max(1.0f32.in_context() - dot(dist, dist) * 0.001f32, 0.0f32.in_context());
        _water <- make_float3!(0.48f32, 0.54f32, 0.36f32) * (p.y().in_context() - 0.6f32) * 0.18f32 * atten;
        set(color, color.in_context() + _water);

        // inverseSqrt mapping
        s <- 600.0f32.in_context() * (1.0f32.in_context() / sqrt(dot(dist, dist)));
        nrm <- (s.in_context() + 8.0f32) / (3.141592f32 * 8.0f32);
        
        _spec_dot <- max(dot(reflect(eye, n), l), 0.0f32.in_context());
        _spec <- pow(_spec_dot, s) * nrm;
        
        set(color, color + make_float3!(_spec));
        color.in_context()
    });
    let get_sea_color = &get_sea_color;

    let get_normal = define_fn("get_normal", |p: Var<Vec3<f32>>, eps: Var<f32>, sea_time: Var<f32>| mdo! {
        ny <- map_detailed.call(p, sea_time);
        nx <- map_detailed.call(make_float3!(p.x().in_context() + eps, p.y(), p.z()), sea_time) - ny;
        nz <- map_detailed.call(make_float3!(p.x(), p.y(), p.z().in_context() + eps), sea_time) - ny;
        normalize(make_float3!(nx, eps, nz))
    });
    let get_normal = &get_normal;

    let height_map_tracing = define_fn("height_map_tracing", |ori: Var<Vec3<f32>>, dir: Var<Vec3<f32>>, sea_time: Var<f32>| mdo! {
        tm <- Var::make_f32(0.0f32);
        tx <- Var::make_f32(1000.0f32);
        hx <- map.call(ori + dir.in_context() * tx, sea_time);
        hm <- map.call(ori, sea_time);
        
        res_t <- Var::make_f32(0.0f32);
        
        if_(hx.gt(0.0f32), mdo! { set(res_t, tx); });
        if_(hx.lte(0.0f32), mdo! {
            (0..32).for_(move |_i, (break_, _)| mdo! {
                tmid <- mix(tm, tx, hm / (hm.in_context() - hx));
                p <- ori + dir.in_context() * tmid;
                hmid <- map.call(p, sea_time);
                
                _t1 <- if_(hmid.lt(0.0f32), mdo! {
                    set(tx, tmid);
                    set(hx, hmid);
                });
                _t2 <- if_(hmid.gte(0.0f32), mdo! {
                    set(tm, tmid);
                    set(hm, hmid);
                });
                
                _t3 <- if_(abs(hmid).lt(1e-3f32), mdo! { break_(); });
            });
            set(res_t, mix(tm, tx, hm / (hm.in_context() - hx)));
        });
        
        Free::Pure(res_t)
    });
    let height_map_tracing = &height_map_tracing;

    let rotate_euler = define_fn("rotate_euler", |dir: Var<Vec3<f32>>, ang: Var<Vec3<f32>>| mdo! {
        a1 <- make_float2!(sin(ang.x()), cos(ang.x()));
        a2 <- make_float2!(sin(ang.y()), cos(ang.y()));
        a3 <- make_float2!(sin(ang.z()), cos(ang.z()));
        
        m0 <- make_float3!(a1.y().in_context() * a3.y() + a1.x().in_context() * a2.x() * a3.x(), a1.y().in_context() * a2.x() * a3.x() + a3.y().in_context() * a1.x(), -a2.y().in_context()*a3.x());
        m1 <- make_float3!(-a2.y().in_context()*a1.x(), a1.y().in_context()*a2.y(), a2.x());
        m2 <- make_float3!(a3.y().in_context()*a1.x()*a2.x() + a1.y().in_context()*a3.x(), a1.x().in_context()*a3.x() - a1.y().in_context()*a3.y()*a2.x(), a2.y().in_context()*a3.y());
        
        make_float3!(
            dot(dir, m0),
            dot(dir, m1),
            dot(dir, m2)
        )
    });
    let rotate_euler = &rotate_euler;

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
                
                time <- i_time.in_context() * 0.3f32; // Mouse input omitted for simplicity
                
                coord <- make_float2!(pos.x(), i_res.y().in_context() - pos.y());
                uv <- coord.in_context() / i_res;
                uv <- uv.in_context() * 2.0f32 - 1.0f32;
                uv <- make_float2!(uv.x() * (i_res.x().in_context() / i_res.y()), uv.y());
                
                ang <- make_float3!(sin(time.in_context() * 3.0f32)*0.1f32, sin(time)*0.2f32 + 0.3f32, time);
                ori <- make_float3!(0.0f32, 3.5f32, time.in_context() * 5.0f32);
                
                dir <- normalize(make_float3!(uv.x(), uv.y(), -2.0f32));
                dir <- make_float3!(dir.x(), dir.y(), dir.z() + length(uv) * 0.14f32);
                dir <- rotate_euler.call(normalize(dir), ang);
                
                // Matches the WGSL constant let sea_time = 1.0 + uniforms.time * SEA_SPEED;
                sea_time <- 1.0f32.in_context() + i_time.in_context() * 0.8f32;
                
                dist_t <- height_map_tracing.call(ori, dir, sea_time);
                p <- ori + dir.in_context() * dist_t;
                dist <- p.in_context() - ori;
                
                eps_nrm <- 0.1f32.in_context() / i_res.x();
                n <- get_normal.call(p, dot(dist, dist) * eps_nrm, sea_time);
                
                light <- normalize(make_float3!(0.0f32, 1.0f32, 0.8f32));
                
                sky_color <- get_sky_color.call(dir);
                sea_color <- get_sea_color.call(p, n, light, dir, dist);
                
                _ss <- smoothstep(-0.02f32.in_context(), 0.0f32.in_context(), dir.y());
                _mix_f <- pow(1.0f32.in_context() - _ss.in_context(), 0.2f32.in_context());
                color <- mix(sky_color, sea_color, make_float3!(_mix_f));
                
                v <- make_float4!(color, 1.0f32);
                Free::Pure(Location::<0, _>::new(v))
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
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
            Event::WindowEvent { event: WindowEvent::Resized(physical_size), .. } => {
                config.width = physical_size.width;
                config.height = physical_size.height;
                surface.configure(&device, &config);
            }
            Event::AboutToWait => window.request_redraw(),
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
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.draw(0..3, 0..1);
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