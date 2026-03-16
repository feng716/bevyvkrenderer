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

    let calc_t = define_fn("calc_t", |time: Var<f32>| mdo! {
        // T(iTime) = sin(iTime*.6)*16.+iTime*1e2
        sin(time.in_context() * 0.6f32) * 16.0f32 + time.in_context() * 100.0f32
    });
    let calc_t = &calc_t;

    let p_func = define_fn("P", |z: Var<f32>| mdo! {
        // P(z) = vec3(cos((z)*.011)*16.+cos((z) * .012)*24., cos((z)*.01)*4., (z))
        make_float3!(
            cos(z.in_context() * 0.011f32) * 16.0f32 + cos(z.in_context() * 0.012f32) * 24.0f32,
            cos(z.in_context() * 0.01f32) * 4.0f32,
            z.in_context()
        )
    });
    let p_func = &p_func;

    let r_mat = define_fn("R", |a: Var<f32>| mdo! {
        // R(a) = mat2(cos(a+vec4(0,33,11,0))) -> Standard 2D rotation matrix
        c <- cos(a);
        s <- sin(a);
        make_mat2!(
            make_float2!(c.in_context(), -s.in_context()),
            make_float2!(s.in_context(), c.in_context())
        )
    });
    let r_mat = &r_mat;

    let boxen = define_fn("boxen", |p: Var<Vec3<f32>>| mdo! {
        // p = abs(fract(p/2e1)*2e1 - 1e1) - 1.;
        p_mod <- abs(fract(p.in_context() / 20.0f32) * 20.0f32 - 10.0f32) - 1.0f32;
        min(p_mod.x(), min(p_mod.y(), p_mod.z()))
    });
    let boxen = &boxen;

    let map = define_fn("map", |p: Var<Vec3<f32>>| mdo! {
        q <- p_func.call(p.z());
        g <- q.y().in_context() - p.y() + 6.0f32;
        
        m <- boxen.call(p);
        
        // p.xy -= q.xy;
        p_xy <- make_float2!(p.x().in_context() - q.x(), p.y().in_context() - q.y());
        
        // Squiggly line along z
        red_offset <- sin(p.z().in_context() / 12.0f32 + make_float2!(0.0f32, 1.3f32)) * 12.0f32;
        red <- length(p_xy.in_context() - red_offset) - 1.0f32;
        
        blue_offset <- sin(p.z().in_context() / 16.0f32 + make_float2!(0.0f32, 0.7f32)) * 16.0f32;
        blue <- length(p_xy.in_context() - blue_offset) - 2.0f32;
        
        e <- min(red, blue);
        
        // Compute lights delta (the w component is 0, so we just calculate RGB)
        l_red <- make_float3!(10.0f32, 2.0f32, 1.0f32) / (0.1f32.in_context() + abs(red));
        l_blue <- make_float3!(1.0f32, 2.0f32, 10.0f32) / (0.1f32.in_context() + abs(blue) / 10.0f32);
        light_delta <- l_red.in_context() + l_blue;
        
        // p = abs(p) -> using the modified xy
        p_mod <- abs(make_float3!(p_xy.x(), p_xy.y(), p.z()));
        
        // tex = abs(length(sin(p*cos(p.yzx/3e1)*4.)/(p*4.))); 
        p_yzx <- make_float3!(p_mod.y(), p_mod.z(), p_mod.x());
        tex_inner <- sin(p_mod.in_context() * cos(p_yzx.in_context() / 30.0f32) * 4.0f32) / (p_mod.in_context() * 4.0f32);
        tex <- abs(length(tex_inner));
        
        // tun = min(32.-p.x - p.y, 24.-p.y);
        tun <- min(32.0f32.in_context() - p_mod.x() - p_mod.y(), 24.0f32.in_context() - p_mod.y());
        
        d <- max(min(m, g), tun) - tex;
        final_d <- min(e, d);
        
        // Return vec4: (distance, light.r, light.g, light.b)
        make_float4!(final_d, light_delta.x(), light_delta.y(), light_delta.z())
    });
    let map = &map;

    let final_wgsl = ShaderCode::new()
        .uniform::<UniformDataVar, UniformData>(0, 0)
        .build_pipeline(|builder, globals| {
            
            builder.vert("vs_main", |BuiltIn(vi, _): BuiltIn<VertexIndex, u32>| mdo! {
                x <- ((vi.in_context() << 1u32) & 2u32).cast::<f32>();
                y <- (vi.in_context() & 2u32).cast::<f32>();
                v <- make_float4!(x.in_context() * 2.0f32 - 1.0f32, 1.0f32 - y.in_context() * 2.0f32, 0.0f32, 1.0f32);
                Free::Pure(BuiltIn::<Position, _>::new(v))
            })

            .frag("fs_main", |BuiltIn(pos, _): BuiltIn<Position, Vec4<f32>>| mdo! {
                let uni = globals.get(UniformDataVar);
                i_res <- uni.resolution().in_context();
                i_time <- uni.time().in_context();
                
                // u = (u-r.xy/2.)/r.y; u.y -=.2; 
                coord <- make_float2!(pos.x(), i_res.y().in_context() - pos.y());
                u <- (coord.in_context() - i_res.in_context() / 2.0f32) / i_res.y().in_context(); // TODO)) without in_context this should work
                u <- make_float2!(u.x(), u.y().in_context() - 0.2f32);
                
                t_val <- calc_t.call(i_time);
                p <- p_func.call(t_val);
                ro <- p.in_context();
                
                p_t2 <- p_func.call(t_val.in_context() + 2.0f32);
                z_vec <- normalize(p_t2.in_context() - p);
                x_vec <- normalize(make_float3!(z_vec.z(), 0.0f32, -z_vec.x().in_context()));
                
                // D = N(vec3(R(sin(T*.005)*.4)*u, 1) * mat3(-X, cross(X, Z), Z));
                r_ang <- sin(t_val.in_context() * 0.005f32) * 0.4f32;
                ru <- r_mat.call(r_ang) * u;
                dir_local <- make_float3!(ru.x(), ru.y(), 1.0f32);
                
                // Constructing mat3 with columns (WGSL column-major)
                c0 <- -x_vec.in_context();
                c1 <- cross(x_vec, z_vec);
                c2 <- z_vec.in_context();
                
                // Vector * Matrix multiplication maps to dot products of columns
                d_dir <- normalize(make_float3!(
                    dot(dir_local, c0),
                    dot(dir_local, c1),
                    dot(dir_local, c2)
                ));
                
                d_tot <- Var::make_f32(0.0f32);
                lights <- make_float3!(0.0f32, 0.0f32, 0.0f32);
                o_acc <- make_float4!(0.0f32);
                
                // Main raymarch loop
                (0..100).for_(move |_i, (break_, _)| mdo! {
                    p_curr <- ro.in_context() + d_dir.in_context() * d_tot;
                    map_res <- map.call(p_curr);
                    s <- map_res.x().in_context();
                    set(d_tot, d_tot.in_context() + s.in_context() * 0.8f32);
                    
                    // Accumulate light delta
                    l_delta <- make_float3!(map_res.y(), map_res.z(), map_res.w());
                    set(lights, lights.in_context() + l_delta);
                    
                    scalar <- 1.0f32.in_context() / max(s.in_context(), 0.01f32.in_context());
                    o_add <- make_float4!(lights.x(), lights.y(), lights.z(), 0.0f32) + make_float4!(scalar);
                    set(o_acc, o_acc.in_context() + o_add);
                });
                
                // Tetrahedron Normal
                h <- Var::make_f32(0.005f32);
                p_final <- ro.in_context() + d_dir.in_context() * d_tot;
                
                k1 <- make_float3!(1.0f32, -1.0f32, -1.0f32);
                k2 <- make_float3!(-1.0f32, -1.0f32, 1.0f32);
                k3 <- make_float3!(-1.0f32, 1.0f32, -1.0f32);
                k4 <- make_float3!(1.0f32, 1.0f32, 1.0f32);
                
                t1 <- map.call(p_final.in_context() + k1.in_context() * h);
                t2 <- map.call(p_final.in_context() + k2.in_context() * h);
                t3 <- map.call(p_final.in_context() + k3.in_context() * h);
                t4 <- map.call(p_final.in_context() + k4.in_context() * h);
                n1 <- k1 * t1.x().in_context();
                n2 <- k2 * t2.x().in_context(); // TODO)) ShaderDSL<'a, Vec4>.a() should be implemented
                n3 <- k3 * t3.x().in_context();
                n4 <- k4 * t4.x().in_context();
                
                n <- normalize(n1.in_context() + n2 + n3 + n4);
                
                // Diffuse
                diff <- 0.1f32.in_context() + max(dot(n, -d_dir.in_context()), 0.0f32.in_context());
                set(o_acc, o_acc.in_context() * diff);
                
                // Reflection march loop
                ref_acc <- make_float4!(0.0f32);
                set(lights, make_float3!(0.0f32, 0.0f32, 0.0f32)); // Reset lights
                
                p_ref <- p_final.in_context() + n.in_context() * 0.05f32;
                d_ref <- reflect(d_dir, n);
                s_ref <- Var::make_f32(0.0f32);
                
                (0..50).for_(move |_i, (break_, _)| mdo! {
                    set(p_ref, p_ref.in_context() + d_ref.in_context() * s_ref);
                    ref_map_res <- map.call(p_ref);
                    set(s_ref, ref_map_res.x().in_context() * 0.8f32);
                    
                    l_delta <- make_float3!(ref_map_res.y(), ref_map_res.z(), ref_map_res.w());
                    set(lights, lights.in_context() + l_delta);
                    
                    scalar <- 1.0f32.in_context() / max(s_ref.in_context(), 0.01f32.in_context());
                    ref_add <- make_float4!(lights.x(), lights.y(), lights.z(), 0.0f32) + make_float4!(scalar);
                    set(ref_acc, ref_acc.in_context() + ref_add);
                });
                
                // Combine and apply post-processing (tanh + exp)
                set(o_acc, o_acc.in_context() + o_acc.in_context() * ref_acc);
                
                exp_inner <- make_float4!(10.0f32, 2.0f32, 1.0f32, 0.0f32) * d_tot.in_context() / 500.0f32;
                o_final <- tanh(o_acc.in_context() / 1e9f32 * exp(exp_inner));
                
                v <- make_float4!(pow(o_final.xyz(), make_float3!(2.2)), 1.);
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