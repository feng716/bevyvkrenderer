mod rendering;
use std::thread;

use rendering::dsl::*;
use shader_macros::ShaderStruct;
use crate::rendering::dsl::{ FuncName, FuncArg };
use std::{collections::HashSet, sync::Arc, time::Instant};
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

#[derive(ShaderStruct, Clone)]
pub struct CameraShader {
    pub pos: Vec3<f32>,
    pub front: Vec3<f32>,
    pub up: Vec3<f32>,
    pub right: Vec3<f32>,
    pub fov: f32,
    pub res: Vec2<f32>,
    pub frame: u32,
}

#[derive(ShaderStruct, Clone)]
pub struct Ray {
    pub ro: Vec3<f32>,
    pub rd: Vec3<f32>,
}

#[derive(ShaderStruct, Clone)]
pub struct Hit {
    pub t: f32,
    pub n: Vec3<f32>,
    pub mat_id: u32,
}

#[derive(Clone, Copy)]
pub struct Onb {
    pub tangent: Var<Vec3<f32>>,
    pub binormal: Var<Vec3<f32>>,
    pub normal: Var<Vec3<f32>>,
}
fn gen_shader() -> String{
    let tea = define_fn("tea", |v0_in: Var<u32>, v1_in: Var<u32>| mdo! {
        v0 <- Var::make_u32(0u32);
        v1 <- Var::make_u32(0u32);
        set(v0, v0_in);
        set(v1, v1_in);
        s0 <- Var::make_u32(0u32);
        (0..4).for_(move |_n, _| mdo! {
            set(s0, s0.in_context() + 0x9e3779b9u32);
            set(v0, v0 + (((v1.in_context() << 4u32) + 0xa341316cu32) ^ (v1 + s0.in_context()) ^ ((v1.in_context() >> 5u32) + 0xc8013ea4u32)));
            set(v1, v1 + (((v0.in_context() << 4u32) + 0xad90777du32) ^ (v0 + s0.in_context()) ^ ((v0.in_context() >> 5u32) + 0x7e95761eu32)));
        });
        Free::Pure(v0)
    });
    let tea = &tea;

    // 2. LCG (Inline Rust Closure - acts like a WGSL macro to mutate `state` in place)
    let lcg = |state: Var<u32>| mdo! {
        set(state, 1664525u32 * state.in_context() + 1013904223u32);
        _masked <- (state.in_context() & 0x00ffffffu32).cast::<f32>();
        _masked.in_context() * (1.0f32 / 16777216.0f32)
    };

    let make_onb = |normal: Var<Vec3<f32>>| mdo! {
        _cond <- abs(normal.x()).gt(abs(normal.z()));
        _v1 <- make_float3!(0.0f32, -normal.z().in_context(), normal.y());
        _v2 <- make_float3!(-normal.y().in_context(), normal.x(), 0.0f32);
        
        binormal <- normalize(select(_v1, _v2, _cond));
        tangent <- normalize(cross(binormal, normal));
        
        Free::Pure(Onb { tangent, binormal, normal }) 
    };

    let to_world = |onb: &Onb, v: Var<Vec3<f32>>| mdo! {
        (v.x().in_context() * onb.tangent + v.y().in_context() * onb.binormal + v.z().in_context() * onb.normal)
    };

    let sample_hemisphere = define_fn("cosine_sample_hemisphere", |u: Var<Vec2<f32>>| mdo! {
        r <- sqrt(u.x());
        phi <- 2.0f32 * 3.14159265f32 * u.y().in_context();
        make_float3!(r * cos(phi), r * sin(phi), sqrt(max(0.0f32, 1.0f32 - u.x().in_context())))
    });
    let sample_hemisphere = &sample_hemisphere;

    let balanced_heuristic = define_fn("balanced_heuristic", |pdf_a: Var<f32>, pdf_b: Var<f32>| mdo! {
        pdf_a / max(pdf_a + pdf_b.in_context(), 1e-4f32)
    });
    let balanced_heuristic = &balanced_heuristic;
    
    let i_bounded_plane = |ray: Var<Ray>, n: Var<Vec3<f32>>, d: f32, bmin: Var<Vec3<f32>>, bmax: Var<Vec3<f32>>, mat: u32, single: bool, hit: Var<Hit>| mdo! {
        denom <- dot(ray.rd(), n);
        
        if_(if single { denom.lt(-1e-5f32) } else { abs(denom).gt(1e-5f32) }, mdo! {
            t <- -(dot(ray.ro(), n) + d) / denom;
            
            if_(t.gt(1e-4f32).and(t.lt(hit.t())), mdo! {
                p <- ray.ro() + ray.rd().in_context() * t;
                
                let bounds_check = p.x().gte(bmin.x()).and(p.x().lte(bmax.x()))
                    .and(p.y().gte(bmin.y())).and(p.y().lte(bmax.y()))
                    .and(p.z().gte(bmin.z())).and(p.z().lte(bmax.z()));
                if_(bounds_check, mdo! {
                    set(hit.t(), t);
                    set(hit.n(), select(n, -n.in_context(), denom.gt(0.0f32)));
                    set(hit.mat_id(), mat);
                });
            });
        });
        Free::Pure(())
    };
    let rot_y = define_fn("rot_y", |p: Var<Vec3<f32>>, a: Var<f32>| mdo! {
        s <- sin(a); c <- cos(a);
        make_float3!(c * p.x().in_context() + s * p.z().in_context(), p.y(), -(s * p.x().in_context()) + c * p.z().in_context())
    });
    let rot_y = &rot_y;

    let i_box_rot = |ray: Var<Ray>, center: Var<Vec3<f32>>, rad: Var<Vec3<f32>>, angle: Var<f32>, mat: u32, hit: Var<Hit>| mdo! {
    
        ro <- rot_y.call(ray.ro().in_context() -center, -angle.in_context());
        rd <- rot_y.call(ray.rd().in_context(), -angle.in_context());
        
        inv_rd <- make_float3!(1.0f32) / rd;
        t0 <- (-rad.in_context() - ro) * inv_rd;
        t1 <- (rad - ro.in_context()) * inv_rd;
        
        tmin <- min(t0, t1);
        tmax <- max(t0, t1);
        
        tnear <- max(max(tmin.x(), tmin.y()), tmin.z());
        tfar <- min(min(tmax.x(), tmax.y()), tmax.z());
        
        if_(tnear.lt(tfar).and(tnear.gt(1e-4f32)).and(tnear.lt(hit.t())), mdo! {
            set(hit.t(), tnear);
            set(hit.mat_id(), mat);
            
            local_p <- (ro + rd.in_context() * tnear) / (rad.in_context() + 1e-5f32);
            
            _n_z <- select(
                make_float3!(0.0f32, 0.0f32, sign(local_p.z())), 
                make_float3!(0.0f32, sign(local_p.y()), 0.0f32), 
                abs(local_p.y()).gt(0.999f32)
            );
            
            n <- select(
                _n_z, 
                make_float3!(sign(local_p.x()), 0.0f32, 0.0f32), 
                abs(local_p.x()).gt(0.999f32)
            );
            
            _hit_n <- rot_y.call(n, angle);
            set(hit.n(), normalize(_hit_n));
        });
        
        Free::Pure(())
    };

    let intersect_scene = define_fn("intersect_scene", |ray: Var<Ray>| mdo! {
        hit <- Hit::new(1e30f32, make_float3!(0.0f32), 999u32);
        bmin <- make_float3!(-1.001f32, -0.001f32, -1.001f32);
        bmax <- make_float3!(1.001f32, 2.001f32, 1.001f32);
        
        a1 <- make_float3!(0.0f32, 1.0f32, 0.0f32);
        a2 <- make_float3!(0.0f32, -1.0f32, 0.0f32);
        a3 <- make_float3!(0.0f32, 0.0f32, 1.0f32);
        a4 <- make_float3!(-1.0f32, 0.0f32, 0.0f32);
        a5 <- make_float3!(1.0f32, 0.0f32, 0.0f32);

        _tmp <- i_bounded_plane(ray, a1, 0.0f32, bmin, bmax, 0u32, false, hit);
        _tmp <- i_bounded_plane(ray, a2, 2.0f32, bmin, bmax, 1u32, false, hit);
        _tmp <- i_bounded_plane(ray, a3, 1.0f32, bmin, bmax, 2u32, false, hit);
        _tmp <- i_bounded_plane(ray, a4, 1.0f32, bmin, bmax, 3u32, true, hit);
        _tmp <- i_bounded_plane(ray, a5, 1.0f32, bmin, bmax, 4u32, true, hit);
        _box1_c <- make_float3!(0.32f32, 0.3f32, 0.28f32);
        _box1_r <- make_float3!(0.3f32, 0.3f32, 0.3f32);
        v <- Var::make_f32(-0.3f32);
        _tmp <- i_box_rot(ray, _box1_c, _box1_r, v, 5u32, hit);
        
        _box2_c <- make_float3!(-0.33f32, 0.6f32, -0.29f32);
        _box2_r <- make_float3!(0.3f32, 0.6f32, 0.3f32);
        v <- Var::make_f32(0.26f32);
        _tmp <- i_box_rot(ray, _box2_c, _box2_r, v, 6u32, hit);
        Free::Pure(hit)
    });
    let intersect_scene = &intersect_scene;
    let get_material = define_fn("get_material", |mat_id: Var<u32>| mdo! {
        color <- make_float3!(0.725f32, 0.710f32, 0.680f32);
        
        if_(mat_id.eq(3u32), mdo! {
            set(color, make_float3!(0.140f32, 0.450f32, 0.091f32));
        });
        
        if_(mat_id.eq(4u32), mdo! {
            set(color, make_float3!(0.630f32, 0.065f32, 0.050f32));
        });
        Free::Pure(color)
    });
    let get_material = &get_material;
    struct CameraVar;
    struct AccumBufferVar;
    ShaderCode::new()
        .uniform::<CameraVar, CameraShader>(0, 0)
        .add_struct::<Ray>()
        .add_struct::<Hit>()
        .storage::<AccumBufferVar, Vec4<f32>, Array1D<Vec4<f32>>, ReadWrite>(0, 1)
        .build_pipeline(|builder, globals| {
            
            // --- 1. COMPUTE SHADER (PATH TRACER) ---
            builder.comp("cs_main", (16, 16, 1), |BuiltIn(id, _): BuiltIn<GlobalInvocationId, Vec3<u32>>| mdo! {
                let cam = globals.get(CameraVar);
                let accum = globals.get(AccumBufferVar);
                
                // Bounds Check
                let bounds_cond = |id_x: Var<f32>, id_y: Var<f32>, res_x: Var<f32>, res_y: Var<f32>| mdo! { 
                    id_x.gte(res_x).or(id_y.gte(res_y)) 
                };

                // 2. Evaluate the BuiltIn integers into floats
                _id_x_f <- id.x().cast::<f32>();
                _id_y_f <- id.y().cast::<f32>();

                // 3. Pass all 4 variables into the context!
                if_(bounds_cond.in_context(_id_x_f, _id_y_f, cam.res().x(), cam.res().y()), mdo! { 
                    return_(); 
                });
                
                buf_idx <- id.y() * cam.res().x().cast::<u32>() + id.x();
                
                // Initialize RNG
                state <- tea.call(id.x() * (cam.frame().in_context() + 1u32), id.y() * (cam.frame().in_context() + 1u32));
                
                // Camera Ray Gen
                fov_rad <- radians(cam.fov());
                p <- (make_float2!(id.x().cast::<f32>(), id.y().cast::<f32>()) + make_float2!(lcg(state), lcg(state))) / cam.res() * 2.0f32 - 1.0f32;
                wi_local <- make_float3!(p.x() * tan(0.5f32 * fov_rad.in_context()), p.y() * tan(0.5f32 * fov_rad.in_context()), -1.0f32);
                wi_world <- normalize(wi_local.x() * cam.right().in_context() - wi_local.y() * cam.up().in_context() - wi_local.z() * cam.front().in_context());
                ray <- Ray::new(cam.pos(), wi_world);

                // Light definitions
                l_pos <- make_float3!(-0.24f32, 1.98f32, 0.16f32);
                l_u <- make_float3!(-0.24f32, 1.98f32, -0.22f32) - l_pos;
                l_v <- make_float3!(0.23f32, 1.98f32, 0.16f32) - l_pos;
                l_em <- make_float3!(17.0f32, 12.0f32, 4.0f32);
                l_area <- length(cross(l_u, l_v));
                l_normal <- normalize(cross(l_u, l_v));

                radiance <- make_float3!(0.0f32);
                beta <- make_float3!(1.0f32);
                pdf_bsdf <- Var::make_f32(0.0f32);

                // THE RAY BOUNCE LOOP
                (0..10).for_(move |_depth, (break_, _)| mdo! {
                    hit <- intersect_scene.call(ray);
                    
                    if_(hit.t().gt(1e20f32), mdo! { break_(); });
                    
                    p_hit <- ray.ro().in_context() + ray.rd().in_context() * hit.t().in_context();
                    cos_wo <- dot(-ray.rd().in_context(), hit.n());
                    
                    if_(cos_wo.lt(1e-4f32), mdo! { break_(); });

                    let hit_light_cond = |p: Var<Vec3<f32>>| mdo! { 
                        p.y().gt(1.97f32)
                        .and(p.x().gt(-0.24f32)).and(p.x().lt(0.23f32))
                        .and(p.z().gt(-0.22f32)).and(p.z().lt(0.16f32))
                    };
                    
                    if_(hit_light_cond.in_context(p_hit), mdo! {
                        if_(_depth.eq(0), mdo! { 
                            set(radiance, radiance.in_context() + l_em); 
                        });
                        
                        if_(!_depth.eq(0), mdo! {
                            pdf_light <- (hit.t().in_context() * hit.t().in_context()) / (l_area.in_context() * cos_wo);
                            mis <- balanced_heuristic.call(pdf_bsdf, pdf_light);
                            set(radiance, radiance + mis.in_context() * beta * l_em);
                        });
                        break_();
                    });

                    albedo <- get_material.call(hit.mat_id());

                    // --- B. Explicit Light Sampling (Next Event Estimation) ---
                    l_p <- l_pos + lcg(state) * l_u + lcg(state) * l_v;
                    wi_light <- normalize(l_p.in_context() - p_hit);
                    d_light <- length(l_p.in_context() - p_hit);
                    
                    shadow_ray <- Ray::new(p_hit + hit.n().in_context() * 1e-4f32, wi_light);
                    shadow_hit <- intersect_scene.call(shadow_ray);
                    
                    cos_wi_l <- dot(wi_light, hit.n());
                    cos_l <- -dot(l_normal, wi_light);
                    
                    if_(shadow_hit.t().gte(d_light.in_context() - 1e-3f32).and(cos_wi_l.gt(1e-4f32)).and(cos_l.gt(1e-4f32)), mdo! {
                        pdf_light_nee <- (d_light * d_light.in_context()) / (l_area * cos_l.in_context());
                        pdf_b <- cos_wi_l.in_context() / 3.14159f32;
                        mis_nee <- balanced_heuristic.call(pdf_light_nee, pdf_b);
                        bsdf <- albedo.in_context() / 3.14159f32 * cos_wi_l;
                        set(radiance, radiance + beta.in_context() * bsdf * mis_nee * l_em / max(pdf_light_nee, 1e-4f32));
                    });

                    // --- C. Sample BSDF ---
                    n <- hit.n().in_context();
                    onb <- make_onb(n);
                    wi_loc <- sample_hemisphere.call(make_float2!(lcg(state), lcg(state)));
                    cos_wi <- abs(wi_loc.z());
                    
                    set(ray, Ray::new(p_hit + hit.n().in_context() * 1e-4f32, to_world(&onb, wi_loc)));
                    set(pdf_bsdf, cos_wi.in_context() / 3.14159f32);
                    set(beta, beta.in_context() * albedo); 
                    
                    // --- D. Russian Roulette ---
                    l <- dot(make_float3!(0.2126f32, 0.7152f32, 0.0722f32), beta);
                    if_(l.eq(0.0f32), mdo! { break_(); });
                    
                    q <- max(l, 0.05f32);
                    if_(lcg(state).gte(q), mdo! { break_(); });
                    set(beta, beta * (1.0f32 / q.in_context()));
                });

                // --- 3. Write Output Accumulation ---
                accum_color <- make_float3!(0.0f32);
                
                f <- cam.frame().in_context();
                if_(f.gt(0u32), mdo! {
                    v <- accum.at(buf_idx.cast::<i32>()).xyz();
                    set(accum_color, v.in_context());
                });
                
                _final_accum <- accum_color + clamp(radiance, make_float3!(0.0f32), make_float3!(30.0f32));
                set(accum.at(buf_idx.cast::<i32>()), make_float4!(_final_accum, 1.0f32));
                
                Free::Pure(())
            })

            // --- 2. VERTEX SHADER (FULLSCREEN) ---
            .vert("vs_main", |BuiltIn(vi, _): BuiltIn<VertexIndex, u32>| mdo! {
                x <- ((vi.in_context() << 1u32) & 2u32).cast::<f32>();
                y <- (vi.in_context() & 2u32).cast::<f32>();
                v <- make_float4!(x.in_context() * 2.0f32 - 1.0f32, 1.0f32 - y.in_context() * 2.0f32, 0.0f32, 1.0f32);
                Free::Pure(BuiltIn::<Position, _>::new(v))
            })

            // --- 3. FRAGMENT SHADER (ACES TONEMAPPING) ---
            .frag("fs_main", |BuiltIn(pos, _): BuiltIn<Position, Vec4<f32>>| mdo! {
                let cam = globals.get(CameraVar);
                let accum = globals.get(AccumBufferVar);
                
                idx <- pos.y().cast::<u32>() * cam.res().x().cast::<u32>() + pos.x().cast::<u32>();
                v <- accum.at(idx.cast::<i32>()).xyz();
                color <- v.in_context() / (cam.frame().in_context() + 1u32).cast::<f32>();
                
                // ACES Map
                a <- Var::make_f32(2.51f32); b <- Var::make_f32(0.03f32); c <- Var::make_f32(2.43f32); d <- Var::make_f32(0.59f32); e <- Var::make_f32(0.14f32);
                mapped <- clamp((color * (a.in_context() * color + b)) / (color * (c.in_context() * color + d) + e), make_float3!(0.0f32), make_float3!(1.0f32));
                
                v <- make_float4!(mapped, 1.0f32);
                Free::Pure(Location::<0, _>::new(v))
            })
        })
}
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    pos: [f32; 3], _p1: u32,
    front: [f32; 3], _p2: u32,
    up: [f32; 3], _p3: u32,
    right: [f32; 3], fov: f32,
    res: [f32; 2], frame: u32, _p4: u32,
}

struct Camera {
    pos: glam::Vec3,
    front: glam::Vec3,
    up: glam::Vec3,
    right: glam::Vec3,
    fov: f32,
}

async fn run() {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new()
        .with_title("Compute Shader Path Tracer")
        .with_inner_size(winit::dpi::LogicalSize::new(1024, 1024))
        .build(&event_loop)
        .unwrap());

    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.unwrap();
    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default(), None).await.unwrap();

    let mut config = surface.get_default_config(&adapter, window.inner_size().width, window.inner_size().height).unwrap();
    surface.configure(&device, &config);

    let child = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(gen_shader)
        .unwrap();

    let s = child.join().unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None, source: wgpu::ShaderSource::Wgsl(s.into()),
    });

    let mut camera = Camera {
        pos: glam::vec3(-0.01, 0.995, 5.0),
        front: glam::vec3(0.0, 0.0, -1.0),
        up: glam::vec3(0.0, 1.0, 0.0),
        right: glam::vec3(1.0, 0.0, 0.0),
        fov: 27.8,
    };

    let mut frame_count = 0u32;
    let mut uniform = CameraUniform {
        pos: camera.pos.into(), _p1: 0,
        front: camera.front.into(), _p2: 0,
        up: camera.up.into(), _p3: 0,
        right: camera.right.into(), fov: camera.fov,
        res: [config.width as f32, config.height as f32], frame: 0, _p4: 0,
    };

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Camera Buffer"),
        size: std::mem::size_of::<CameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let create_accum_buffer = |device: &wgpu::Device, w: u32, h: u32| {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Accum Buffer"),
            size: (w * h * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        })
    };
    let mut accum_buffer = create_accum_buffer(&device, config.width, config.height);

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1, visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            },
        ],
    });

    let mut bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None, layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: accum_buffer.as_entire_binding() },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None, bind_group_layouts: &[&bind_group_layout], push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute"), layout: Some(&pipeline_layout),
        module: &shader, entry_point: "cs_main",
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render"), layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module: &shader, entry_point: "fs_main",
            targets: &[Some(config.format.into())],
        }),
        primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
        depth_stencil: None, multisample: wgpu::MultisampleState::default(), multiview: None,
    });

    let mut keys = HashSet::new();
    let mut last_time = Instant::now();

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(key), state, .. }, .. } => {
                    if state == ElementState::Pressed { keys.insert(key); } else { keys.remove(&key); }
                }
                WindowEvent::Resized(size) => {
                    config.width = size.width.max(1); config.height = size.height.max(1);
                    surface.configure(&device, &config);
                    
                    // Add &device here as well
                    accum_buffer = create_accum_buffer(&device, config.width, config.height);
                    
                    bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None, layout: &bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: accum_buffer.as_entire_binding() },
                        ],
                    });
                    frame_count = 0;
                }
                WindowEvent::RedrawRequested => {
                    let dt = last_time.elapsed().as_secs_f32();
                    last_time = Instant::now();
                    let mut is_dirty = false;
                    let speed = 2.0 * dt;
                    let rot_speed = 1.0 * dt;

                    // Camera Controller
                    if keys.contains(&KeyCode::KeyW) { camera.pos += camera.front * speed; is_dirty = true; }
                    if keys.contains(&KeyCode::KeyS) { camera.pos -= camera.front * speed; is_dirty = true; }
                    if keys.contains(&KeyCode::KeyA) { camera.pos -= camera.right * speed; is_dirty = true; }
                    if keys.contains(&KeyCode::KeyD) { camera.pos += camera.right * speed; is_dirty = true; }
                    if keys.contains(&KeyCode::Space) { camera.pos += camera.up * speed; is_dirty = true; }
                    if keys.contains(&KeyCode::ShiftLeft) { camera.pos -= camera.up * speed; is_dirty = true; }
                    
                    if keys.contains(&KeyCode::ArrowLeft) {
                        let rot = glam::Mat3::from_axis_angle(camera.up, rot_speed);
                        camera.front = rot * camera.front; camera.right = rot * camera.right; is_dirty = true;
                    }
                    if keys.contains(&KeyCode::ArrowRight) {
                        let rot = glam::Mat3::from_axis_angle(camera.up, -rot_speed);
                        camera.front = rot * camera.front; camera.right = rot * camera.right; is_dirty = true;
                    }

                    if is_dirty { frame_count = 0; }

                    uniform.pos = camera.pos.into();
                    uniform.front = camera.front.into();
                    uniform.up = camera.up.into();
                    uniform.right = camera.right.into();
                    uniform.res = [config.width as f32, config.height as f32];
                    uniform.frame = frame_count;
                    queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniform));

                    let output = surface.get_current_texture().unwrap();
                    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                    { // Compute Pass
                        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
                        cpass.set_pipeline(&compute_pipeline);
                        cpass.set_bind_group(0, &bind_group, &[]);
                        cpass.dispatch_workgroups((config.width + 15) / 16, (config.height + 15) / 16, 1);
                    }
                    { // Presentation Pass
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None, color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view, resolve_target: None,
                                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                            })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
                        });
                        rpass.set_pipeline(&render_pipeline);
                        rpass.set_bind_group(0, &bind_group, &[]);
                        rpass.draw(0..3, 0..1);
                    }

                    queue.submit(std::iter::once(encoder.finish()));
                    output.present();
                    frame_count += 1;
                }
                _ => {}
            }
            Event::AboutToWait => window.request_redraw(),
            _ => {}
        }
    }).unwrap();
}

const STACK_SIZE: usize = 4 * 1024 * 1024;
fn main(){
    pollster::block_on(run());
}