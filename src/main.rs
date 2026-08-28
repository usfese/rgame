mod ecs;
mod game;
mod renderer;
mod string_id;

use crate::game::Game;

fn main() {
    let mut game: Game = Game::new("RGame");
    game.game_loop();
}
