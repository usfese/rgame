mod ecs;
mod game;
mod render;
mod renderer;
mod string_id;

use crate::game::Game;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let mut game = Game::new("RGame");
    event_loop.run_app(&mut game).expect("run app");
}
