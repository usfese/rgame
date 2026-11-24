//! 使用内置顶点和颜色绘制三角形。
//!
//! shader 修改后，使用 `glslc --target-env=vulkan1.3` 重新生成相邻的 `.spv` 文件。

use std::io::Cursor;

use ash::{Device, vk};

use super::{FrameContext, Pass, PassKind};

const VERTEX_SHADER: &[u8] = include_bytes!("shaders/triangle.vert.spv");
const FRAGMENT_SHADER: &[u8] = include_bytes!("shaders/triangle.frag.spv");

pub struct TrianglePass {
    pipeline: Option<TrianglePipeline>,
}

impl TrianglePass {
    pub fn new() -> Self {
        Self { pipeline: None }
    }
}

impl Pass for TrianglePass {
    fn name(&self) -> &'static str {
        "triangle"
    }

    fn kind(&self) -> PassKind {
        PassKind::Render {
            load: vk::AttachmentLoadOp::LOAD,
            clear_color: [0.0; 4],
        }
    }

    fn execute(&mut self, ctx: &mut FrameContext<'_>) {
        let pipeline = self.pipeline.get_or_insert_with(|| {
            TrianglePipeline::new(ctx.device, ctx.color_format).expect("triangle pipeline")
        });
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: ctx.extent.width as f32,
            height: ctx.extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: ctx.extent,
        };
        unsafe {
            ctx.device.cmd_bind_pipeline(
                ctx.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline.pipeline,
            );
            ctx.device
                .cmd_set_viewport(ctx.command_buffer, 0, &[viewport]);
            ctx.device
                .cmd_set_scissor(ctx.command_buffer, 0, &[scissor]);
            ctx.device.cmd_draw(ctx.command_buffer, 3, 1, 0, 0);
        }
    }

    fn on_resize(&mut self, _extent: vk::Extent2D, format: vk::Format) {
        if self
            .pipeline
            .as_ref()
            .is_some_and(|pipeline| pipeline.format != format)
        {
            self.pipeline = None;
        }
    }
}

struct TrianglePipeline {
    device: Device,
    format: vk::Format,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl TrianglePipeline {
    fn new(device: &Device, format: vk::Format) -> Result<Self, vk::Result> {
        let vertex_code =
            ash::util::read_spv(&mut Cursor::new(VERTEX_SHADER)).expect("vertex SPIR-V");
        let fragment_code =
            ash::util::read_spv(&mut Cursor::new(FRAGMENT_SHADER)).expect("fragment SPIR-V");
        unsafe {
            let vertex = device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&vertex_code),
                None,
            )?;
            let fragment = match device.create_shader_module(
                &vk::ShaderModuleCreateInfo::default().code(&fragment_code),
                None,
            ) {
                Ok(module) => module,
                Err(error) => {
                    device.destroy_shader_module(vertex, None);
                    return Err(error);
                }
            };
            let result = Self::create_pipeline(device, format, vertex, fragment);
            device.destroy_shader_module(fragment, None);
            device.destroy_shader_module(vertex, None);
            result
        }
    }

    unsafe fn create_pipeline(
        device: &Device,
        format: vk::Format,
        vertex: vk::ShaderModule,
        fragment: vk::ShaderModule,
    ) -> Result<Self, vk::Result> {
        unsafe {
            let layout =
                device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default(), None)?;
            let stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .module(vertex)
                    .name(c"main"),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .module(fragment)
                    .name(c"main"),
            ];
            let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
            let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            let viewport = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            let raster = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .cull_mode(vk::CullModeFlags::NONE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .line_width(1.0);
            let multisample = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            let blend_attachment = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA);
            let blend = vk::PipelineColorBlendStateCreateInfo::default()
                .attachments(std::slice::from_ref(&blend_attachment));
            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic =
                vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
            let formats = [format];
            let mut rendering =
                vk::PipelineRenderingCreateInfo::default().color_attachment_formats(&formats);
            let info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&vertex_input)
                .input_assembly_state(&assembly)
                .viewport_state(&viewport)
                .rasterization_state(&raster)
                .multisample_state(&multisample)
                .color_blend_state(&blend)
                .dynamic_state(&dynamic)
                .layout(layout)
                .push_next(&mut rendering);
            let pipeline =
                match device.create_graphics_pipelines(vk::PipelineCache::null(), &[info], None) {
                    Ok(pipelines) => pipelines[0],
                    Err((pipelines, error)) => {
                        for pipeline in pipelines {
                            device.destroy_pipeline(pipeline, None);
                        }
                        device.destroy_pipeline_layout(layout, None);
                        return Err(error);
                    }
                };
            Ok(Self {
                device: device.clone(),
                format,
                layout,
                pipeline,
            })
        }
    }
}

impl Drop for TrianglePipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
        }
    }
}
