use crate::ecs::entity::{self, EntityId};
use std::any::Any;
const INVALID: u32 = u32::MAX;

pub trait Component: Any + Send + Sync {}

pub struct Storage<T: Component> {
    sparse: Vec<u32>,
    entities: Vec<EntityId>,
    data: Vec<T>,
}

impl<T: Component> Storage<T> {
    fn new() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            data: Vec::new(),
        }
    }
    fn index_of(&self, entity: EntityId) -> Option<usize> {
        let slot = entity.slot() as usize;
        let &index = self.sparse.get(slot)?;
        if index == INVALID {
            return None;
        }
        let index = index as usize;
        if *self.entities.get(index)? != entity {
            return None;
        }
        Some(index)
    }
    fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }

    fn insert(&mut self, entity: EntityId, component: T) {
        let slot = entity.slot() as usize;
        if self.sparse.len() <= slot {
            let new_len = (self.sparse.len() * 2).max(slot + 1);
            self.sparse.resize(new_len, INVALID);
        }
        if self.contains(entity) {
            let index = self.index_of(entity).unwrap();
            self.data.insert(index, component);
            return;
        }
        let index = self.data.len();
        self.sparse[slot] = index as u32;
        self.entities.push(entity);
        self.data.push(component);
    }
}
