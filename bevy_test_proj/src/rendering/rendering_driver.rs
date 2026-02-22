use std::{ffi::CString, mem::ManuallyDrop};

use crate::rendering::rendering_loop::RenderingLoop;
use anyhow::{bail, Context};
use ash::vk::{self, AccessFlags, Extent2D, Offset2D};
use bevy::{
    ecs::system::NonSendMarker,
    prelude::*,
    window::{PrimaryWindow, WindowResized},
    winit::WINIT_WINDOWS,
};
use raw_window_handle::HasWindowHandle;
struct ConvertToCstr {
    #[expect(unused, reason = "C strs should not be dropped while being referenced")]
    intmed: Vec<CString>,
    ptrs: Vec<*const std::os::raw::c_char>,
}
impl ConvertToCstr {
    fn new<T>(i: &[T]) -> Self
    where
        T: AsRef<[u8]>,
    {
        let s: Vec<CString> = i
            .iter()
            .map(|u| CString::new(u.as_ref()).unwrap())
            .collect();
        Self {
            ptrs: s.iter().map(|u| u.as_ptr()).collect(),
            intmed: s,
        }
    }
    fn ptrs(&self) -> *const *const i8 {
        self.ptrs.as_ptr()
    }
}
#[derive(Resource)]
struct VkRenderingResource {
    // dropping order is as declared
    lp: ManuallyDrop<RenderingLoop>,
    driver: ManuallyDrop<VkRenderingDriver>,
}

struct Test<'a> {
    t: &'a VkRenderingDriver,
}

impl Drop for VkRenderingResource {
    fn drop(&mut self) {
        unsafe {
            self.lp.drop(&self.driver);
        }
        if let Some((mes, ins)) = &self.driver.debug_messenger {
            unsafe {
                ins.destroy_debug_utils_messenger(*mes, None);
            }
        }
        unsafe {
            let instance =
                ash::khr::surface::Instance::new(&self.driver.entry, &self.driver.instance);
            instance.destroy_surface(self.driver.surface, None);
        }
        unsafe {
            self.driver.device.destroy_device(None);
            self.driver.instance.destroy_instance(None);
        }
    }
}
pub struct VkRenderingDriver {
    entry: ash::Entry,
    instance: ash::Instance,
    debug_messenger: Option<(vk::DebugUtilsMessengerEXT, ash::ext::debug_utils::Instance)>,
    pub device: ash::Device,
    pub graphic_queue: vk::Queue,
    surface: vk::SurfaceKHR,
    phy_device: vk::PhysicalDevice,
}
impl VkRenderingDriver {
    pub fn new(window_handle: raw_window_handle::WindowHandle) -> anyhow::Result<Self> {
        // let val_layers;
        let entry;
        unsafe {
            entry = ash::Entry::load().context("failed to create Entry")?;
            // val_layers = entry.enumerate_instance_layer_properties()?;
        }
        // println!("{:#?}", val_layers);

        let enabled_layers = ["VK_LAYER_KHRONOS_validation"];
        let enabled_layers_cstrs = ConvertToCstr::new(&enabled_layers);

        let instance_required_extensions = [
            ash::ext::debug_utils::NAME.to_bytes(),
            ash::khr::surface::NAME.to_bytes(),
            ash::khr::win32_surface::NAME.to_bytes(),
        ];
        let required_extensions_cstrs = ConvertToCstr::new(&instance_required_extensions);

        let app_info = vk::ApplicationInfo {
            api_version: vk::make_api_version(0, 1, 3, 0),
            ..Default::default()
        };

        let create_info = vk::InstanceCreateInfo {
            p_application_info: &app_info,
            enabled_layer_count: enabled_layers.len() as u32,
            pp_enabled_layer_names: enabled_layers_cstrs.ptrs(),
            enabled_extension_count: instance_required_extensions.len() as u32,
            pp_enabled_extension_names: required_extensions_cstrs.ptrs(),
            ..Default::default()
        };

        let instance;
        unsafe {
            instance = entry.create_instance(&create_info, None)?;
        }

        let phy_devices;
        unsafe {
            phy_devices = instance.enumerate_physical_devices()?;
            // println!("{:#?}", instance.get_physical_device_queue_family_properties(phy_devices[0]));
            // let a = instance.enumerate_device_extension_properties(phy_devices[0])?;
            // for i in a {
            //     if let Ok(x) = std::ffi::CStr::from_ptr(i.extension_name.as_ptr()).to_str(){
            //         if  x == "VK_EXT_descriptor_buffer" {
            //             println!("{:#?}", i);
            //         }
            //     }
            // } 
        }

        let queue_priorities = 1.0;
        let queue_create_info = vk::DeviceQueueCreateInfo {
            queue_family_index: 0,
            queue_count: 1,
            p_queue_priorities: &queue_priorities,
            ..Default::default()
        };
        let enabled_features = vk::PhysicalDeviceFeatures::default();

        let device_required_extensions = [ash::khr::swapchain::NAME.to_bytes()];
        let device_required_extensions_cstrs = ConvertToCstr::new(&device_required_extensions);
        // device-level layer is now deprecated
        let device_create_info = vk::DeviceCreateInfo {
            p_queue_create_infos: &queue_create_info,
            queue_create_info_count: 1,
            p_enabled_features: &enabled_features,
            enabled_extension_count: device_required_extensions.len() as u32,
            pp_enabled_extension_names: device_required_extensions_cstrs.ptrs(),
            ..Default::default()
        };
        // unsafe {
        //     println!(
        //         "{:?}",
        //         instance
        //             .get_physical_device_format_properties(phy_devices[0], vk::Format::D32_SFLOAT)
        //     );
        // }
        let device;

        unsafe {
            device = instance.create_device(phy_devices[0], &device_create_info, None)?;
            
            if let Some(x) = entry.try_enumerate_instance_version()? {
                println!("{}.{}.{}", vk::api_version_major(x), vk::api_version_minor(x), vk::api_version_patch(x));
            }
        }
        let surface;
        {
            let instance = ash::khr::win32_surface::Instance::new(&entry, &instance);
            let raw_window_handle::RawWindowHandle::Win32(window_handle) = window_handle.as_raw()
            else {
                unimplemented!()
            };
            let create_info = vk::Win32SurfaceCreateInfoKHR {
                hinstance: window_handle
                    .hinstance
                    .ok_or(anyhow::anyhow!("No hInstance"))?
                    .get(),
                hwnd: window_handle.hwnd.get(),
                ..Default::default()
            };
            unsafe { surface = instance.create_win32_surface(&create_info, None)? }
        }

        Ok(VkRenderingDriver {
            debug_messenger: None,
            graphic_queue: unsafe { device.get_device_queue(0, 0) },
            surface: surface,
            entry: entry,
            instance: instance,
            device: device,
            phy_device: phy_devices[0],
        })
    }
    pub fn create_swapchain(
        &self,
    ) -> anyhow::Result<(vk::SwapchainKHR, ash::khr::swapchain::Device)> {
        let cap;
        {
            let instance = ash::khr::surface::Instance::new(&self.entry, &self.instance);
            unsafe {
                cap = instance
                    .get_physical_device_surface_capabilities(self.phy_device, self.surface)?;
                // println!("{:#?}", cap);
                // println!(
                //     "{:#?}",
                //     instance.get_physical_device_surface_present_modes(phy_devices[0], surface)?
                // );
                // println!(
                //     "{:#?}",
                //     instance.get_physical_device_surface_formats(phy_devices[0], surface)?
                // );
            }
        }
        let swapchain_device;
        {
            let create_info = vk::SwapchainCreateInfoKHR {
                surface: self.surface,
                min_image_count: 3,
                image_format: vk::Format::R8G8B8A8_SRGB,
                image_color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
                image_extent: cap.max_image_extent,
                image_array_layers: 1,
                image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
                image_sharing_mode: vk::SharingMode::EXCLUSIVE,
                pre_transform: cap.current_transform,
                composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
                present_mode: vk::PresentModeKHR::MAILBOX,
                clipped: vk::TRUE,
                old_swapchain: vk::SwapchainKHR::null(),
                ..Default::default()
            };
            swapchain_device = ash::khr::swapchain::Device::new(&self.instance, &self.device);
            unsafe {
                Ok((
                    swapchain_device.create_swapchain(&create_info, None)?,
                    swapchain_device,
                ))
            }
        }
    }

    pub fn framebuffers(
        &self,
        images_views: &Vec<vk::ImageView>,
        rp: vk::RenderPass,
        extent: vk::Extent2D,
    ) -> anyhow::Result<Vec<vk::Framebuffer>> {
        images_views
            .iter()
            .map(|e| {
                let attachments = [*e];
                let create_info = vk::FramebufferCreateInfo {
                    render_pass: rp,
                    attachment_count: 1,
                    p_attachments: attachments.as_ptr(),
                    width: extent.width,
                    height: extent.height,
                    layers: 1,
                    ..Default::default()
                };
                unsafe { Ok(self.device.create_framebuffer(&create_info, None)?) }
            })
            .collect()
    }
    pub fn setup_debug_utils(&mut self) -> anyhow::Result<()> {
        unsafe extern "system" fn vk_debug_callback(
            message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
            message_type: vk::DebugUtilsMessageTypeFlagsEXT,
            p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
            _p_user_data: *mut std::ffi::c_void,
        ) -> vk::Bool32 {
            if message_severity >= vk::DebugUtilsMessageSeverityFlagsEXT::WARNING {
                println!(
                    "[{:?}]:\n{:?}: Message Id Number: {:?}\n\t{}",
                    message_severity,
                    message_type,
                    (*p_callback_data).message_id_number,
                    std::ffi::CStr::from_ptr((*p_callback_data).p_message)
                        .to_str()
                        .unwrap()
                );
            }
            vk::FALSE
        }
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT {
            message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            pfn_user_callback: Some(vk_debug_callback),
            ..Default::default()
        };
        let instance = ash::ext::debug_utils::Instance::new(&self.entry, &self.instance);
        unsafe {
            self.debug_messenger = Some((
                instance.create_debug_utils_messenger(&create_info, None)?,
                instance,
            ));
        }
        Ok(())
    }

    pub fn setup_render(
        _non_send_marker: NonSendMarker,
        primary_window: Query<Entity, With<PrimaryWindow>>,
        bevy_window: Query<&mut Window, With<PrimaryWindow>>,
        mut cmd: Commands,
    ) {
        error!("setup_render1");
        
        WINIT_WINDOWS.with_borrow(|winit_res| {
            if let Some(res) = winit_res.get_window(primary_window.single().unwrap()) {
                // let res = res.window_handle().unwrap().as_raw();
                let mut driver = VkRenderingDriver::new(res.window_handle().unwrap()).unwrap();
                driver.setup_debug_utils().unwrap();
                let bevy_window = bevy_window.single().unwrap();
                let s = bevy_window.resolution.size();
                let lp = RenderingLoop::new(
                    &driver,
                    Extent2D {
                        width: s.x as u32,
                        height: s.y as u32,
                    },
                )
                .unwrap();
                cmd.insert_resource(VkRenderingResource {
                    driver: ManuallyDrop::new(driver),
                    lp: ManuallyDrop::new(lp),
                });
            }
        });
        error!("setup_render2");
    }

    pub fn load_shader<P>(&self, path: P) -> anyhow::Result<vk::ShaderModule>
    where
        P: AsRef<std::path::Path>,
    {
        let mut file = std::fs::File::open(path)?;
        let data = ash::util::read_spv(&mut file)?;
        let create_info = vk::ShaderModuleCreateInfo::default().code(&data);
        unsafe { Ok(self.device.create_shader_module(&create_info, None)?) }
    }

    pub fn pipeline(
        &self,
        extent: vk::Extent2D,
        rp: vk::RenderPass,
        uniforms_layout: vk::PipelineLayout,
    ) -> anyhow::Result<vk::Pipeline> {
        let vert = vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::VERTEX,
            module: self.load_shader("shaders/a_vert.spv")?,
            p_name: c"main".as_ptr() as *const i8,
            ..Default::default()
        };
        let frag = vk::PipelineShaderStageCreateInfo {
            stage: vk::ShaderStageFlags::FRAGMENT,
            module: self.load_shader("shaders/a_frag.spv")?,
            p_name: c"main".as_ptr() as *const i8,
            ..Default::default()
        };
        let stages = [vert, frag];

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_states_create_info = vk::PipelineDynamicStateCreateInfo {
            dynamic_state_count: dynamic_states.len() as u32,
            p_dynamic_states: dynamic_states.as_ptr(),
            ..Default::default()
        };
        let vertex_input_create_info = vk::PipelineVertexInputStateCreateInfo::default();

        let viewport_state = vk::PipelineViewportStateCreateInfo {
            viewport_count: 1,
            scissor_count: 1,
            ..Default::default()
        };

        let rasterizer = vk::PipelineRasterizationStateCreateInfo {
            depth_clamp_enable: vk::FALSE,
            rasterizer_discard_enable: vk::FALSE,
            polygon_mode: vk::PolygonMode::FILL,
            line_width: 1.,
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::CLOCKWISE,
            depth_bias_enable: vk::FALSE,
            ..Default::default()
        };

        let assembly = vk::PipelineInputAssemblyStateCreateInfo {
            topology: vk::PrimitiveTopology::TRIANGLE_LIST,
            primitive_restart_enable: vk::FALSE,
            ..Default::default()
        };

        let multisampling = vk::PipelineMultisampleStateCreateInfo {
            sample_shading_enable: vk::FALSE,
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            ..Default::default()
        };

        let attachment_color_blend = vk::PipelineColorBlendAttachmentState {
            color_write_mask: vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
            blend_enable: vk::FALSE,
            ..Default::default()
        };

        let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
            logic_op_enable: vk::FALSE,
            attachment_count: 1,
            p_attachments: &attachment_color_blend,
            ..Default::default()
        };

        // let uniforms = vk::PipelineLayoutCreateInfo {
        //     ..Default::default()
        // };
        // let uniforms_layout;
        // unsafe { uniforms_layout = self.device.create_pipeline_layout(&uniforms, None)?; }

        // let rp = self.render_pass(uniforms_layout)?;

        let create_info = vk::GraphicsPipelineCreateInfo {
            stage_count: 2,
            p_stages: stages.as_ptr(),
            p_vertex_input_state: &vertex_input_create_info,
            p_input_assembly_state: &assembly,
            p_viewport_state: &viewport_state,
            p_rasterization_state: &rasterizer,
            p_multisample_state: &multisampling,
            p_color_blend_state: &color_blend_state,
            p_dynamic_state: &dynamic_states_create_info,
            layout: uniforms_layout,
            render_pass: rp,
            subpass: 0,
            ..Default::default()
        };
        let res = unsafe {
            match self.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[create_info],
                None,
            ) {
                Err((ps, err_code)) => bail!("Err {}", err_code),
                Ok(a) => a[0],
            }
        };
        unsafe {
            self.device.destroy_shader_module(vert.module, None);
            self.device.destroy_shader_module(frag.module, None);
        }
        Ok(res)
    }

    pub fn render_pass(
        &self,
        uniforms_layout: vk::PipelineLayout,
    ) -> anyhow::Result<vk::RenderPass> {
        let color_attachment = vk::AttachmentDescription {
            format: vk::Format::R8G8B8A8_SRGB,
            samples: vk::SampleCountFlags::TYPE_1,
            load_op: vk::AttachmentLoadOp::CLEAR,
            store_op: vk::AttachmentStoreOp::STORE,
            stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
            stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
            ..Default::default()
        };

        let attachment_reference = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };

        let subpass = vk::SubpassDescription {
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            color_attachment_count: 1,
            p_color_attachments: &attachment_reference,
            ..Default::default()
        };

        let deps = [vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: AccessFlags::empty(),
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            ..Default::default()
        }];
        let create_info = vk::RenderPassCreateInfo {
            attachment_count: 1,
            p_attachments: &color_attachment,
            subpass_count: 1,
            p_subpasses: &subpass,
            p_dependencies: deps.as_ptr(),
            dependency_count: deps.len() as u32,
            ..Default::default()
        };
        unsafe { Ok(self.device.create_render_pass(&create_info, None)?) }
    }

    pub fn cmd_pool(&self, queue_index: u32) -> anyhow::Result<vk::CommandPool> {
        let create_info = vk::CommandPoolCreateInfo {
            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            queue_family_index: queue_index,
            ..Default::default()
        };
        Ok(unsafe { self.device.create_command_pool(&create_info, None)? })
    }

    pub fn cmd_buffer(
        &self,
        command_pool: &vk::CommandPool,
        ct: u32,
    ) -> anyhow::Result<Vec<vk::CommandBuffer>> {
        let create_info = vk::CommandBufferAllocateInfo {
            command_pool: *command_pool,
            level: vk::CommandBufferLevel::PRIMARY,
            command_buffer_count: ct,
            ..Default::default()
        };
        unsafe { Ok(self.device.allocate_command_buffers(&create_info)?) }
    }

    pub fn record(
        &self,
        command_buffer: vk::CommandBuffer,
        fb: vk::Framebuffer,
        rp: vk::RenderPass,
        extent: vk::Extent2D,
        p: vk::Pipeline,
        v: vk::Viewport,
        scissor: vk::Rect2D,
    ) -> anyhow::Result<()> {
        let cmd_begin_create_info = vk::CommandBufferBeginInfo {
            ..Default::default()
        };
        let clear_color = vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0., 0., 0., 1.],
            },
        };
        let render_pass_begin_info = vk::RenderPassBeginInfo {
            render_pass: rp,
            framebuffer: fb,
            render_area: vk::Rect2D {
                offset: Offset2D { x: 0, y: 0 },
                extent: extent,
            },
            clear_value_count: 1,
            p_clear_values: &clear_color,
            ..Default::default()
        };

        unsafe {
            self.device
                .begin_command_buffer(command_buffer, &cmd_begin_create_info)?;
            self.device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
            self.device
                .cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, p);
            self.device.cmd_set_viewport(command_buffer, 0, &[v]);
            self.device.cmd_set_scissor(command_buffer, 0, &[scissor]);
            self.device.cmd_draw(command_buffer, 3, 1, 0, 0);
            self.device.cmd_end_render_pass(command_buffer);
            self.device.end_command_buffer(command_buffer)?;
        }
        Ok(())
    }

    pub fn semaphore(&self) -> anyhow::Result<vk::Semaphore> {
        let create_info = vk::SemaphoreCreateInfo {
            ..Default::default()
        };
        unsafe { Ok(self.device.create_semaphore(&create_info, None)?) }
    }

    pub fn fence(&self) -> anyhow::Result<vk::Fence> {
        let create_info = vk::FenceCreateInfo {
            flags: vk::FenceCreateFlags::SIGNALED,
            ..Default::default()
        };
        unsafe { Ok(self.device.create_fence(&create_info, None)?) }
    }

    pub fn get_images_from_swapchain(
        &self,
        (swapchain, dev): &(vk::SwapchainKHR, ash::khr::swapchain::Device),
    ) -> anyhow::Result<(Vec<vk::Image>, Vec<vk::ImageView>)> {
        let images = unsafe { dev.get_swapchain_images(*swapchain)? };
        let image_views: Vec<vk::ImageView> = images
            .iter()
            .map(|a| {
                let create_info = vk::ImageViewCreateInfo {
                    image: *a,
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: vk::Format::R8G8B8A8_SRGB,
                    components: vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    },
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    ..Default::default()
                };
                unsafe { self.device.create_image_view(&create_info, None) }
            })
            .collect::<anyhow::Result<Vec<_>, _>>()?;
        Ok((images, image_views))
    }
}

fn render_system(mut res: ResMut<VkRenderingResource>, mut reader: MessageReader<WindowResized>) {
    let VkRenderingResource { lp, driver } = &mut *res;
    if reader.len() > 0 {
        for i in reader.read() {
            lp.render(
                &driver,
                Some(vk::Extent2D {
                    width: i.width as u32,
                    height: i.height as u32,
                }),
            )
            .unwrap();
        }
    } else {
        lp.render(&driver, None).unwrap();
    }
}

pub struct VkRenderingPlugin;
impl Plugin for VkRenderingPlugin {
    fn build(&self, app: &mut App) {
        app //.add_systems(Update, egui_system)
            .add_systems(Startup, VkRenderingDriver::setup_render)
            .add_systems(Update, render_system);
    }
}
