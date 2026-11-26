use std::{
    any::{Any, TypeId},
    collections::HashMap,
    hash::Hash,
};

use crate::ecs::world::World;

struct EventChannel<T> {
    pending: Vec<T>,
    listeners: Vec<Box<dyn FnMut(&T, &mut World)>>,
}

trait ErasedChannel {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn dispatch(&mut self, world: &mut World);
}

impl<T: 'static> ErasedChannel for EventChannel<T> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn dispatch(&mut self, world: &mut World) {
        for event in self.pending.drain(..) {
            for listener in &mut self.listeners {
                listener(&event, world);
            }
        }
    }
}

pub struct EventEmitter {
    channels: HashMap<TypeId, Box<dyn ErasedChannel>>,
}

impl EventEmitter {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    fn channels<T: 'static>(&mut self) -> &mut EventChannel<T> {
        let channel = self.channels.entry(TypeId::of::<T>()).or_insert_with(|| {
            Box::new(EventChannel::<T> {
                pending: Vec::new(),
                listeners: Vec::new(),
            })
        });
        channel
            .as_any_mut()
            .downcast_mut()
            .expect("事件类型和频道类型不一致。")
    }
}
