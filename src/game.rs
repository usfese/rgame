//! 游戏模块。
//! 包含[`Game`]。
use std::sync::Arc;

use crate::{
    ecs::{scheduler::Scheduler, world::World},
    render::{pass::PassChain, passes::ClearPass},
    renderer::Renderer,
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, Window, WindowId},
};

/// 表示游戏的struct。
pub struct Game {
    name: String,
    scheduler: Scheduler,
    world: World,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
}
impl Game {
    pub fn new(name: &str) -> Game {
        let s_name = String::from(name);
        Game {
            name: s_name,
            scheduler: Scheduler::new(),
            world: World::new(),
            window: None,
            renderer: None,
        }
    }

    pub fn new_with_world(name: &str, world: World) -> Game {
        let s_name = String::from(name);
        Game {
            name: s_name,
            scheduler: Scheduler::new(),
            world: world,
            window: None,
            renderer: None,
        }
    }

    pub fn replace_world(&mut self, world: World) -> World {
        std::mem::replace(&mut self.world, world)
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

// 和winit库有关的代码
impl ApplicationHandler for Game {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attributes = Window::default_attributes()
            .with_title(self.name.as_str())
            .with_inner_size(winit::dpi::LogicalSize::new(
                f64::from(1920),
                f64::from(1080),
            ))
            .with_fullscreen(Some(Fullscreen::Borderless(None)));
        let window = Arc::new(event_loop.create_window(attributes).unwrap());

        let mut renderer = Renderer::new(&window).expect("Vulkan renderer");

        let mut passes = PassChain::new();
        passes.push(ClearPass::new([0.05, 0.07, 0.12, 1.0]));
        *renderer.passes_mut() = passes;

        self.window = Some(Arc::clone(&window));
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.render();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                let toggle = event.state == ElementState::Pressed
                    && !event.repeat
                    && event.physical_key == PhysicalKey::Code(KeyCode::F11);

                if toggle {
                    if let Some(window) = &self.window {
                        let target = match window.fullscreen() {
                            Some(_) => None,
                            None => Some(Fullscreen::Borderless(window.current_monitor())),
                        };

                        window.set_fullscreen(target);
                    }
                }
            }

            WindowEvent::MouseInput { .. } => {}

            WindowEvent::CursorMoved { .. } => {}

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
