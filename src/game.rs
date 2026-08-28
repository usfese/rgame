use crate::renderer::Renderer;
use winit::{
    application::ApplicationHandler,
    event::{KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

pub struct Game {
    name: String,
    is_running: bool,
    renderer: Option<Renderer>,
}
impl Game {
    pub fn new(name: &str) -> Game {
        let s_name = String::from(name);
        Game {
            name: s_name,
            is_running: false,
            renderer: None,
        }
    }
    fn handle_input(&mut self) {
        todo!();
    }
    fn update_all(&mut self) {
        todo!();
    }
    fn render(&mut self) {
        todo!();
    }

    pub fn game_loop(&mut self) {
        self.is_running = true;
        while self.is_running {
            self.handle_input();
            self.update_all();
            self.render();
        }
    }
    fn exit(&mut self) {
        self.is_running = false;
    }
}

impl ApplicationHandler for Game {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let attributes = Window::default_attributes()
            .with_title("Ash - Example")
            .with_inner_size(winit::dpi::LogicalSize::new(
                f64::from(1920),
                f64::from(1080),
            ));
        let window = event_loop.create_window(attributes).unwrap();

        self.renderer = Some(Renderer::new(&window).unwrap());
    }
    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                println!("resized: {}x{}", size.width, size.height);

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
                // 处理键盘
            }

            WindowEvent::MouseInput { state, button, .. } => {
                // 处理鼠标按键
            }

            WindowEvent::CursorMoved { position, .. } => {
                // 处理鼠标移动
            }

            _ => {}
        }
    }
}
