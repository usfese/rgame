//! 渲染 pass 抽象。
//!
//! 一次渲染由若干 [`Pass`] 顺序执行组成。渲染器只负责帧的获取、提交和呈现，
//! 每个 pass 负责自己那一段命令记录：
//! - [`PassKind::Render`]：动态渲染 pass，渲染器负责 `vkCmdBeginRendering` /
//!   `vkCmdEndRendering` 以及 swapchain 图像的布局转换；
//! - [`PassKind::Compute`]：计算 pass，不绑定附件；
//! - [`PassKind::Custom`]：自定义逻辑 pass，直接拿到裸命令缓冲。
//!
//! 目前按注册顺序串行执行，不做帧图或依赖排序。

use ash::{Device, vk};

/// 单个 pass 执行时可见的帧信息。
pub struct FrameContext<'a> {
    pub device: &'a Device,
    /// 本帧使用的命令缓冲，已经处于录制状态。
    pub command_buffer: vk::CommandBuffer,
    /// 帧槽位下标，范围是 `0..MAX_FRAME_LATENCY`。
    pub frame_index: usize,
    /// 本帧获取到的 swapchain 图像下标。
    pub image_index: u32,
    /// 当前 swapchain 的像素尺寸。
    pub extent: vk::Extent2D,
    /// 本帧 swapchain 图像的颜色视图。
    pub color_view: vk::ImageView,
    /// 本帧 swapchain 图像的颜色格式。
    pub color_format: vk::Format,
}

/// pass 的类型，决定渲染器如何封装它。
#[derive(Clone, Copy, Debug)]
pub enum PassKind {
    /// 动态渲染 pass，写入 swapchain 颜色附件。
    Render {
        load: vk::AttachmentLoadOp,
        clear_color: [f32; 4],
    },
    /// 计算 pass，不绑定任何附件。
    Compute,
    /// 自定义 pass，渲染器不额外介入。
    Custom,
}

/// 一段可插入渲染流程的命令记录逻辑。
pub trait Pass {
    /// 调试用名字。
    fn name(&self) -> &'static str;

    /// 决定渲染器如何封装本次执行。
    fn kind(&self) -> PassKind;

    /// 在已录制的命令缓冲里记录本 pass 的命令。
    ///
    /// `Render` 类型调用时，动态渲染已经开始，颜色附件已按 `load` 绑定。
    fn execute(&mut self, ctx: &mut FrameContext<'_>);

    /// swapchain 重建后回调，用于重建分辨率相关资源。
    fn on_resize(&mut self, _extent: vk::Extent2D) {}
}

/// 按注册顺序保存的 pass 列表。
#[derive(Default)]
pub struct PassChain {
    passes: Vec<Box<dyn Pass>>,
}

impl PassChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push<P: Pass + 'static>(&mut self, pass: P) {
        self.passes.push(Box::new(pass));
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Box<dyn Pass>> {
        self.passes.iter_mut()
    }

    pub fn on_resize(&mut self, extent: vk::Extent2D) {
        for pass in &mut self.passes {
            pass.on_resize(extent);
        }
    }
}
