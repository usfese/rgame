mod ecs;
mod game;
mod renderer;
mod string_id;

use crate::{ecs::world::World, game::Game};

fn main() {
    let mut game: Game = Game::new("RGame");
    game.game_loop();
    // let world = world::new();
}
