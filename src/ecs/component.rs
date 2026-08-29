use crate::ecs::entity::{self, EntityId};
use std::any::Any;
const INVALID: u32 = u32::MAX;

pub trait Component: Any + Send + Sync {}

pub trait ErasedStorage: Any + Send + Sync {
    fn remove_entity(&mut self, entity: EntityId);
    fn as_any(&self) -> &dyn Any;
    fn as_mut_any(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub struct Storage<T: Component> {
    sparse: Vec<u32>,
    entities: Vec<EntityId>,
    data: Vec<T>,
}

impl<T: Component> Storage<T> {
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            data: Vec::new(),
        }
    }
    pub fn index_of(&self, entity: EntityId) -> Option<usize> {
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
    pub fn contains(&self, entity: EntityId) -> bool {
        self.index_of(entity).is_some()
    }

    pub fn insert(&mut self, entity: EntityId, component: T) {
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
    pub fn get(&self, entity: EntityId) -> Option<&T> {
        let index = self.index_of(entity)?;
        Some(&self.data[index])
    }
    pub fn get_mut(&mut self, entity: EntityId) -> Option<&mut T> {
        let index = self.index_of(entity)?;
        Some(&mut self.data[index])
    }
}

impl<T: Component> ErasedStorage for Storage<T> {
    fn remove_entity(&mut self, entity: EntityId) {
        let Some(index) = self.index_of(entity) else {
            return;
        };
        let last = self.data.len() - 1;
        self.sparse[entity.slot() as usize] = INVALID;
        self.entities.swap_remove(index);
        self.data.swap_remove(index);
        if index != last {
            let moved_entity = self.entities[index];
            self.sparse[moved_entity.slot() as usize] = index as u32;
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}
