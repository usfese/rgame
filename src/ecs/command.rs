use crate::ecs::world::World;

pub struct WorldCommands {
    queue: Vec<Box<dyn FnOnce(&mut World)>>,
}

impl WorldCommands {
    pub fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub fn push<F>(&mut self, command: F)
    where
        F: FnOnce(&mut World) + 'static,
    {
        self.queue.push(Box::new(command));
    }

    pub fn apply(&mut self, world: &mut World) {
        for command in self.queue.drain(..) {
            command(world);
        }
    }
}

impl Default for WorldCommands {
    fn default() -> Self {
        Self::new()
    }
}
