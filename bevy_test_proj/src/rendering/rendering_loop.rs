use ash::vk;

use crate::rendering::rendering_driver::VkRenderingDriver;
pub struct RenderingLoop {
    swapchain: (vk::SwapchainKHR, ash::khr::swapchain::Device),
    image_available_semas: [vk::Semaphore; Self::FRAMES_IN_FLIGHT],
    render_finished_semas: [vk::Semaphore; Self::FRAMES_IN_FLIGHT],
    in_flight_fences: [vk::Fence; Self::FRAMES_IN_FLIGHT],
    command_pool: vk::CommandPool,
    command_buffer: [vk::CommandBuffer; Self::FRAMES_IN_FLIGHT],
    image_views: Vec<vk::ImageView>,
    fbs: Vec<vk::Framebuffer>,
    rp: vk::RenderPass,
    p: vk::Pipeline,
    extent: vk::Extent2D,
    frames: usize,
    images: Vec<vk::Image>,
}

impl RenderingLoop {
    const FRAMES_IN_FLIGHT: usize = 2;
    pub fn new(driver: &VkRenderingDriver, extent: vk::Extent2D) -> anyhow::Result<Self> {
        let cp = driver.cmd_pool(0)?;
        let uniforms = vk::PipelineLayoutCreateInfo {
            ..Default::default()
        };
        let uniforms_layout;
        unsafe {
            uniforms_layout = driver.device.create_pipeline_layout(&uniforms, None)?;
        }
        let rp = driver.render_pass(uniforms_layout)?;
        fn create_seq<T, A>(mut f: T) -> anyhow::Result<[A; RenderingLoop::FRAMES_IN_FLIGHT]>
        where
            T: FnMut(i32) -> anyhow::Result<A>,
        {
            let vec: anyhow::Result<Vec<A>> = (0..RenderingLoop::FRAMES_IN_FLIGHT)
                .map(|x| f(x as i32))
                .collect();
            Ok(vec?.try_into().map_err(|_| ()).unwrap())
        }
        let s = driver.create_swapchain()?;
        let (images, image_views) = driver.get_images_from_swapchain(&s)?;
        Ok(Self {
            image_available_semas: create_seq(|_| driver.semaphore())?,
            render_finished_semas: create_seq(|_| driver.semaphore())?,
            in_flight_fences: create_seq(|_| driver.fence())?,
            command_buffer: driver
                .cmd_buffer(&cp, 2)?
                .try_into()
                .map_err(|_| ())
                .unwrap(),
            command_pool: cp,
            fbs: driver.framebuffers(&image_views, rp, extent)?,
            image_views: image_views,
            images: images,
            p: {
                let a = driver.pipeline(extent, rp, uniforms_layout)?;
                unsafe {
                    driver.device.destroy_pipeline_layout(uniforms_layout, None);
                }
                a
            },
            rp: rp,
            extent: extent,
            frames: 0,
            swapchain: s,
        })
    }

    pub fn render(
        &mut self,
        driver: &VkRenderingDriver,
        resize_extent: Option<ash::vk::Extent2D>,
    ) -> anyhow::Result<()> {
        unsafe {
            driver
                .device
                .wait_for_fences(&[self.in_flight_fences[self.frames]], true, u64::MAX)?;
            if let Some(e) = resize_extent {
                self.recreate_swapchain(driver, e)?;
                return Ok(());
            }
            let (swapchain, dev) = &self.swapchain;
            let idx = match dev.acquire_next_image(
                *swapchain,
                u64::MAX,
                self.image_available_semas[self.frames],
                vk::Fence::null(),
            ) {
                Ok((idx, _)) => idx,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    if let Some(e) = resize_extent {
                        self.recreate_swapchain(driver, e)?;
                    }
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            };
            driver
                .device
                .reset_fences(&[self.in_flight_fences[self.frames]])?;
            driver.record(
                self.command_buffer[self.frames],
                self.fbs[idx as usize],
                self.rp,
                self.extent,
                self.p,
                vk::Viewport {
                    x: 0.,
                    y: 0.,
                    width: self.extent.width as f32,
                    height: self.extent.height as f32,
                    min_depth: 0.,
                    max_depth: 1.,
                },
                vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.extent,
                },
            )?;

            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let submit_i = vk::SubmitInfo {
                p_wait_semaphores: &self.image_available_semas[self.frames],
                p_wait_dst_stage_mask: wait_stages.as_ptr(),
                wait_semaphore_count: 1,
                command_buffer_count: 1,
                p_command_buffers: &self.command_buffer[self.frames],
                signal_semaphore_count: 1,
                p_signal_semaphores: &self.render_finished_semas[self.frames],
                ..Default::default()
            };
            driver.device.queue_submit(
                driver.graphic_queue,
                &[submit_i],
                self.in_flight_fences[self.frames],
            )?;

            let swapchains = [*swapchain];
            let present = vk::PresentInfoKHR {
                wait_semaphore_count: 1,
                p_wait_semaphores: &self.render_finished_semas[self.frames],
                swapchain_count: 1,
                p_swapchains: swapchains.as_ptr(),
                p_image_indices: &idx,
                ..Default::default()
            };
            let res = dev.queue_present(driver.graphic_queue, &present);
            match res {
                Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    if res.is_err() {
                        // Trigger recreation logic here if needed, or rely on next frame's acquire to fail
                        if let Some(e) = resize_extent {
                            self.recreate_swapchain(driver, e)?;
                        }
                    }
                }
                Err(e) => return Err(e.into()),
            }
            self.frames = (self.frames + 1) % Self::FRAMES_IN_FLIGHT;
        }
        Ok(())
    }

    pub unsafe fn cleanup_swapchain(&self, driver: &VkRenderingDriver) {
        self.fbs
            .iter()
            .for_each(|e| driver.device.destroy_framebuffer(*e, None));
        self.image_views.iter().for_each(|e| {
            driver.device.destroy_image_view(*e, None);
        });
        let (swapchain, dev) = &self.swapchain;
        dev.destroy_swapchain(*swapchain, None);
    }

    pub unsafe fn recreate_swapchain(
        &mut self,
        driver: &VkRenderingDriver,
        extent: vk::Extent2D,
    ) -> anyhow::Result<()> {
        driver.device.device_wait_idle()?;
        self.cleanup_swapchain(driver);
        let s = driver.create_swapchain()?;
        let (images, image_views) = driver.get_images_from_swapchain(&s)?;
        let fb = driver.framebuffers(&image_views, self.rp, extent)?;
        self.swapchain.0 = s.0;
        self.images = images;
        self.image_views = image_views;
        self.fbs = fb;
        self.extent = extent;
        Ok(())
    }

    pub unsafe fn drop(&mut self, driver: &VkRenderingDriver) {
        driver.device.device_wait_idle().unwrap();
        self.cleanup_swapchain(driver);
        driver.device.destroy_pipeline(self.p, None);
        driver.device.destroy_render_pass(self.rp, None);
        // self.images.iter().for_each(|e|{
        //     driver.device.destroy_image(*e, None);
        // });
        self.image_available_semas
            .iter()
            .for_each(|e| driver.device.destroy_semaphore(*e, None));
        self.render_finished_semas
            .iter()
            .for_each(|e| driver.device.destroy_semaphore(*e, None));
        self.in_flight_fences
            .iter()
            .for_each(|e| driver.device.destroy_fence(*e, None));
        driver.device.destroy_command_pool(self.command_pool, None);
    }
}
