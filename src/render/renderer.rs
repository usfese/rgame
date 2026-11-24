//! Vulkan 渲染后端。
//!
//! 负责实例/设备/交换链与帧同步，本身不决定画什么：
//! 每帧按顺序执行 [`PassChain`] 中的 pass。
//!
//! 帧流程：
//! 1. 等待当前帧槽位的 fence；
//! 2. 获取交换链图像；
//! 3. 把图像从 `UNDEFINED` 转到 `COLOR_ATTACHMENT_OPTIMAL`；
//! 4. 依次执行 pass（`Render` 类型的 pass 由渲染器包裹 `vkCmdBeginRendering`）；
//! 5. 把图像转到 `PRESENT_SRC_KHR`；
//! 6. 提交并呈现。

#![warn(
    clippy::use_self,
    deprecated_in_future,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unused_qualifications
)]

use std::{
    borrow::Cow, error::Error, ffi, os::raw::c_char,
};

use ash::{
    Device, Entry, Instance,
    ext::debug_utils,
    khr::{surface, swapchain},
    vk::{self, Handle},
};

use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

use crate::render::pass::{FrameContext, PassChain, PassKind};

/// 同时在飞的帧数上限。
pub const MAX_FRAME_LATENCY: usize = 3;

const VALIDATION_LAYER: &ffi::CStr = c"VK_LAYER_KHRONOS_validation";

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    // SAFETY: Vulkan 保证回调期间 `p_callback_data` 有效。
    let callback_data = unsafe { *p_callback_data };
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        unsafe { ffi::CStr::from_ptr(callback_data.p_message_id_name) }.to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        unsafe { ffi::CStr::from_ptr(callback_data.p_message) }.to_string_lossy()
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}

pub struct Renderer {
    entry: Entry,
    instance: Instance,
    debug_utils_loader: debug_utils::Instance,
    debug_call_back: vk::DebugUtilsMessengerEXT,
    surface_loader: surface::Instance,
    surface: vk::SurfaceKHR,

    device: Device,
    pdevice: vk::PhysicalDevice,
    queue_family_index: u32,
    present_queue: vk::Queue,

    swapchain_loader: swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_format: vk::Format,
    swapchain_extent: vk::Extent2D,
    swapchain_images: Vec<vk::Image>,
    swapchain_views: Vec<vk::ImageView>,

    pool: vk::CommandPool,
    command_buffers: [vk::CommandBuffer; MAX_FRAME_LATENCY],
    present_complete: [vk::Semaphore; MAX_FRAME_LATENCY],
    render_complete: Vec<vk::Semaphore>,
    in_flight_fences: [vk::Fence; MAX_FRAME_LATENCY],
    frame_index: usize,

    passes: PassChain,
}

impl Renderer {
    pub fn new(window: &Window) -> Result<Self, Box<dyn Error>> {
        unsafe {
            let entry = Entry::linked();

            let instance_version = entry.try_enumerate_instance_version()?.unwrap_or(vk::API_VERSION_1_0);
            if instance_version < vk::API_VERSION_1_3 {
                return Err(format!(
                    "Vulkan 1.3 required, instance reports {}.{}.{}",
                    vk::api_version_major(instance_version),
                    vk::api_version_minor(instance_version),
                    vk::api_version_patch(instance_version),
                )
                .into());
            }

            let layer_properties = entry.enumerate_instance_layer_properties()?;
            let validation_available = layer_properties.iter().any(|layer| {
                layer
                    .layer_name_as_c_str()
                    .is_ok_and(|name| name == VALIDATION_LAYER)
            });

            let layer_names: Vec<*const c_char> = if validation_available {
                vec![VALIDATION_LAYER.as_ptr()]
            } else {
                Vec::new()
            };

            let display_handle = window.display_handle()?.as_raw();
            let window_handle = window.window_handle()?.as_raw();

            let mut extension_names = ash_window::enumerate_required_extensions(display_handle)
                .map_err(|code| format!("surface extensions unavailable: {code:?}"))?
                .to_vec();
            extension_names.push(debug_utils::NAME.as_ptr());

            let application_name = c"rgame";
            let app_info = vk::ApplicationInfo::default()
                .application_name(application_name)
                .application_version(0)
                .engine_name(application_name)
                .engine_version(0)
                .api_version(vk::API_VERSION_1_3);

            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_layer_names(&layer_names)
                .enabled_extension_names(&extension_names);

            let instance = entry.create_instance(&create_info, None)?;

            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_utils_loader = debug_utils::Instance::new(&entry, &instance);
            let debug_call_back = debug_utils_loader.create_debug_utils_messenger(&debug_info, None)?;

            let surface = ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)?;
            let surface_loader = surface::Instance::new(&entry, &instance);

            let (pdevice, queue_family_index) =
                pick_physical_device(&instance, &surface_loader, surface)?;

            let mut features13 = vk::PhysicalDeviceVulkan13Features::default();
            let mut features2 = vk::PhysicalDeviceFeatures2::default().push_next(&mut features13);
            instance.get_physical_device_features2(pdevice, &mut features2);

            if features13.dynamic_rendering == vk::FALSE {
                return Err("device does not support dynamic rendering".into());
            }

            let priorities = [1.0f32];
            let queue_info = vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities);

            let device_extensions = [swapchain::NAME.as_ptr()];
            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_extension_names(&device_extensions)
                .push_next(&mut features13);

            let device = instance.create_device(pdevice, &device_create_info, None)?;
            let present_queue = device.get_device_queue(queue_family_index, 0);
            let swapchain_loader = swapchain::Device::new(&instance, &device);

            let mut renderer = Self {
                entry,
                instance,
                debug_utils_loader,
                debug_call_back,
                surface_loader,
                surface,
                device,
                pdevice,
                queue_family_index,
                present_queue,
                swapchain_loader,
                swapchain: vk::SwapchainKHR::null(),
                swapchain_format: vk::Format::UNDEFINED,
                swapchain_extent: vk::Extent2D { width: 0, height: 0 },
                swapchain_images: Vec::new(),
                swapchain_views: Vec::new(),
                pool: vk::CommandPool::null(),
                command_buffers: [vk::CommandBuffer::null(); MAX_FRAME_LATENCY],
                present_complete: [vk::Semaphore::null(); MAX_FRAME_LATENCY],
                render_complete: Vec::new(),
                in_flight_fences: [vk::Fence::null(); MAX_FRAME_LATENCY],
                frame_index: 0,
                passes: PassChain::new(),
            };

            renderer.create_command_resources()?;
            renderer.create_swapchain(vk::SwapchainKHR::null())?;
            renderer.create_sync_objects()?;

            let initial_extent = renderer.swapchain_extent;
            renderer.passes.on_resize(initial_extent, renderer.swapchain_format);

            Ok(renderer)
        }
    }

    /// 等待设备空闲后替换 pass 链，确保旧 pass 的 GPU 资源不再被使用。
    pub fn set_passes(&mut self, passes: PassChain) {
        unsafe { self.device.device_wait_idle().expect("device idle") };
        self.passes = passes;
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.swapchain_extent
    }

    unsafe fn create_command_resources(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let pool_info = vk::CommandPoolCreateInfo::default()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(self.queue_family_index);
            self.pool = self.device.create_command_pool(&pool_info, None)?;

            let allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(MAX_FRAME_LATENCY as u32);

            let buffers = self.device.allocate_command_buffers(&allocate_info)?;
            self.command_buffers = buffers.try_into().expect("command buffer count mismatch");

            Ok(())
        }
    }

    unsafe fn create_sync_objects(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            for semaphore in &mut self.present_complete {
                *semaphore = self.device.create_semaphore(&semaphore_info, None)?;
            }

            let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
            for fence in &mut self.in_flight_fences {
                *fence = self.device.create_fence(&fence_info, None)?;
            }

            self.create_render_complete_semaphores()?;

            Ok(())
        }
    }

    unsafe fn create_render_complete_semaphores(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe {
            let semaphore_info = vk::SemaphoreCreateInfo::default();

            self.render_complete = (0..self.swapchain_images.len())
                .map(|_| self.device.create_semaphore(&semaphore_info, None))
                .collect::<Result<Vec<_>, _>>()?;

            Ok(())
        }
    }

    unsafe fn create_swapchain(&mut self, old_swapchain: vk::SwapchainKHR) -> Result<(), Box<dyn Error>> {
        unsafe {
            let capabilities = self
                .surface_loader
                .get_physical_device_surface_capabilities(self.pdevice, self.surface)?;

            let formats = self
                .surface_loader
                .get_physical_device_surface_formats(self.pdevice, self.surface)?;

            let preferred = formats.iter().find(|format| {
                matches!(
                    format.format,
                    vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB
                ) && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            });
            let format = preferred.copied().unwrap_or_else(|| formats[0]);

            let extent = if capabilities.current_extent.width != u32::MAX {
                capabilities.current_extent
            } else {
                let min = capabilities.min_image_extent;
                let max = capabilities.max_image_extent;
                vk::Extent2D {
                    width: self.swapchain_extent.width.clamp(min.width.min(1), max.width),
                    height: self.swapchain_extent.height.clamp(min.height.min(1), max.height),
                }
            };

            let mut image_count = capabilities.min_image_count + 1;
            if capabilities.max_image_count > 0 {
                image_count = image_count.min(capabilities.max_image_count);
            }

            let present_modes = self
                .surface_loader
                .get_physical_device_surface_present_modes(self.pdevice, self.surface)?;
            let present_mode = present_modes
                .iter()
                .copied()
                .find(|mode| *mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);

            let pre_transform = if capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                capabilities.current_transform
            };

            let create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(self.surface)
                .min_image_count(image_count)
                .image_format(format.format)
                .image_color_space(format.color_space)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .old_swapchain(old_swapchain);

            let new_swapchain = self.swapchain_loader.create_swapchain(&create_info, None)?;
            let images = self.swapchain_loader.get_swapchain_images(new_swapchain)?;

            let views = images
                .iter()
                .map(|&image| {
                    let view_info = vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format.format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(1)
                                .layer_count(1),
                        );
                    self.device.create_image_view(&view_info, None)
                })
                .collect::<Result<Vec<_>, _>>()?;

            if !self.swapchain.is_null() {
                for view in self.swapchain_views.drain(..) {
                    self.device.destroy_image_view(view, None);
                }
                self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            }

            self.swapchain = new_swapchain;
            self.swapchain_format = format.format;
            self.swapchain_extent = extent;
            self.swapchain_images = images;
            self.swapchain_views = views;

            Ok(())
        }
    }

    /// 重建交换链、图像视图和呈现信号量。
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.swapchain_extent = vk::Extent2D { width, height };

        unsafe {
            self.device.device_wait_idle().expect("device idle");
            self.recreate_swapchain();
        }
    }

    unsafe fn recreate_swapchain(&mut self) {
        unsafe {
            for semaphore in self.render_complete.drain(..) {
                self.device.destroy_semaphore(semaphore, None);
            }

            if let Err(error) = self.create_swapchain(self.swapchain) {
                panic!("swapchain recreation failed: {error}");
            }

            self.create_render_complete_semaphores()
                .expect("recreate render complete semaphores");

            let extent = self.swapchain_extent;
            self.passes.on_resize(extent, self.swapchain_format);
        }
    }

    /// 渲染并呈现一帧。
    pub fn render(&mut self) {
        match unsafe { self.render_frame() } {
            Ok(()) => {}
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                unsafe {
                    self.device.device_wait_idle().expect("device idle");
                    self.recreate_swapchain();
                }
            }
            Err(error) => panic!("frame failed: {error:?}"),
        }
    }

    unsafe fn render_frame(&mut self) -> Result<(), vk::Result> {
        unsafe {
            let frame = self.frame_index;
            let fence = self.in_flight_fences[frame];

            self.device.wait_for_fences(&[fence], true, u64::MAX)?;

            let acquire = self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.present_complete[frame],
                vk::Fence::null(),
            );

            let image_index = match acquire {
                Ok((index, _)) => index,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.device.device_wait_idle()?;
                    self.recreate_swapchain();
                    return Ok(());
                }
                Err(error) => return Err(error),
            };

            self.device.reset_fences(&[fence])?;

            let command_buffer = self.command_buffers[frame];
            self.device.reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::empty(),
            )?;

            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device.begin_command_buffer(command_buffer, &begin_info)?;

            let image = self.swapchain_images[image_index as usize];
            let extent = self.swapchain_extent;
            let color_view = self.swapchain_views[image_index as usize];

            transition_image(
                &self.device,
                command_buffer,
                image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );

            let device = self.device.clone();
            let mut ctx = FrameContext {
                device: &device,
                command_buffer,
                frame_index: frame,
                image_index,
                extent,
                color_view,
                color_format: self.swapchain_format,
            };

            let mut rendered_color = false;
            for pass in self.passes.iter_mut() {
                match pass.kind() {
                    PassKind::Render { load, clear_color } => {
                        if rendered_color {
                            let barrier = vk::MemoryBarrier::default()
                                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                                .dst_access_mask(
                                    vk::AccessFlags::COLOR_ATTACHMENT_READ
                                        | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                                );
                            device.cmd_pipeline_barrier(
                                command_buffer,
                                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                vk::DependencyFlags::empty(),
                                &[barrier],
                                &[],
                                &[],
                            );
                        }
                        let attachment = vk::RenderingAttachmentInfo::default()
                            .image_view(color_view)
                            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                            .load_op(load)
                            .store_op(vk::AttachmentStoreOp::STORE)
                            .clear_value(vk::ClearValue {
                                color: vk::ClearColorValue { float32: clear_color },
                            });

                        let area = vk::Rect2D {
                            offset: vk::Offset2D { x: 0, y: 0 },
                            extent,
                        };

                        let rendering_info = vk::RenderingInfo::default()
                            .render_area(area)
                            .layer_count(1)
                            .color_attachments(std::slice::from_ref(&attachment));

                        device.cmd_begin_rendering(command_buffer, &rendering_info);
                        pass.execute(&mut ctx);
                        device.cmd_end_rendering(command_buffer);
                        rendered_color = true;
                    }
                    PassKind::Compute | PassKind::Custom => pass.execute(&mut ctx),
                }
            }

            transition_image(
                &self.device,
                command_buffer,
                image,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );

            self.device.end_command_buffer(command_buffer)?;

            let wait_semaphores = [self.present_complete[frame]];
            let signal_semaphores = [self.render_complete[image_index as usize]];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [command_buffer];

            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);

            self.device
                .queue_submit(self.present_queue, &[submit_info], fence)?;

            let swapchains = [self.swapchain];
            let image_indices = [image_index];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            let present_result = self.swapchain_loader.queue_present(self.present_queue, &present_info);

            self.frame_index = (frame + 1) % MAX_FRAME_LATENCY;

            match present_result {
                Ok(_) => Ok(()),
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                    self.device.device_wait_idle()?;
                    self.recreate_swapchain();
                    Ok(())
                }
                Err(error) => Err(error),
            }
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            drop(std::mem::take(&mut self.passes));

            if !self.pool.is_null() {
                self.device.destroy_command_pool(self.pool, None);
            }

            for semaphore in self.present_complete.iter().copied() {
                if !semaphore.is_null() {
                    self.device.destroy_semaphore(semaphore, None);
                }
            }

            for semaphore in self.render_complete.drain(..) {
                self.device.destroy_semaphore(semaphore, None);
            }

            for fence in self.in_flight_fences.iter().copied() {
                if !fence.is_null() {
                    self.device.destroy_fence(fence, None);
                }
            }

            for view in self.swapchain_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }

            if !self.swapchain.is_null() {
                self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            }

            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);

            if !self.debug_call_back.is_null() {
                self.debug_utils_loader
                    .destroy_debug_utils_messenger(self.debug_call_back, None);
            }

            self.instance.destroy_instance(None);
        }
    }
}

unsafe fn transition_image(
    device: &Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let (src_stage, src_access, dst_stage, dst_access) = match (old_layout, new_layout) {
        (vk::ImageLayout::UNDEFINED, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => (
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::AccessFlags::empty(),
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        (vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::PRESENT_SRC_KHR) => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::empty(),
        ),
        _ => panic!("unsupported image layout transition {old_layout:?} -> {new_layout:?}"),
    };

    let barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .src_access_mask(src_access)
        .dst_access_mask(dst_access)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .level_count(1)
                .layer_count(1),
        );

    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
    }
}

unsafe fn pick_physical_device(
    instance: &Instance,
    surface_loader: &surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32), Box<dyn Error>> {
    unsafe {
        let devices = instance.enumerate_physical_devices()?;

        let mut best: Option<(vk::PhysicalDevice, u32, u32)> = None;

        for device in devices {
            let properties = instance.get_physical_device_properties(device);

            if properties.api_version < vk::API_VERSION_1_3 {
                continue;
            }

            let families = instance.get_physical_device_queue_family_properties(device);

            let Some(index) = families.iter().enumerate().position(|(index, family)| {
                family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    && surface_loader
                        .get_physical_device_surface_support(device, index as u32, surface)
                        .unwrap_or(false)
            }) else {
                continue;
            };

            let score = match properties.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 2,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
                _ => 0,
            };

            if best.is_none_or(|(_, _, current)| score > current) {
                best = Some((device, index as u32, score));
            }
        }

        best.map(|(device, index, _)| (device, index))
            .ok_or_else(|| "no Vulkan 1.3 device with graphics and present support".into())
    }
}
