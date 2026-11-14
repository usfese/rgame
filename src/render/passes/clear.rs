//! 清屏 pass。
//!
//! 不绘制任何几何体，只把 swapchain 图像清成统一颜色，用来验证帧循环。

use crate::render::pass::{FrameContext, Pass, PassKind};

pub struct ClearPass {
    pub color: [f32; 4],
}

impl ClearPass {
    pub fn new(color: [f32; 4]) -> Self {
        Self { color }
    }
}

impl Pass for ClearPass {
    fn name(&self) -> &'static str {
        "clear"
    }

    fn kind(&self) -> PassKind {
        PassKind::Render {
            load: ash::vk::AttachmentLoadOp::CLEAR,
            clear_color: self.color,
        }
    }

    fn execute(&mut self, _ctx: &mut FrameContext<'_>) {}
}
