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

        if let Some(index) = self.index_of(entity) {
            self.data[index] = component;
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

    pub fn remove(&mut self, entity: EntityId) -> Option<T> {
        let slot = entity.slot() as usize;

        if slot >= self.sparse.len() {
            return None;
        }

        let index = self.sparse[slot] as usize;

        if index == INVALID as usize {
            return None;
        }

        if self.entities[index] != entity {
            return None;
        }

        let last_index = self.data.len() - 1;

        self.entities.swap_remove(index);
        let component = self.data.swap_remove(index);

        if index != last_index {
            let moved_entity = self.entities[index];
            self.sparse[moved_entity.slot() as usize] = index as u32;
        }

        self.sparse[slot] = INVALID;

        Some(component)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::entity::EntityId;

    #[derive(Debug, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    impl Component for Position {}

    fn entity(slot: u32) -> EntityId {
        EntityId::new(slot, 0)
    }

    fn position(x: f32, y: f32) -> Position {
        Position { x, y }
    }

    #[test]
    fn new_storage_is_empty() {
        let storage = Storage::<Position>::new();

        assert!(storage.get(entity(0)).is_none());
        assert!(storage.get(entity(1)).is_none());
    }

    #[test]
    fn insert_and_get() {
        let mut storage = Storage::new();
        let e = entity(0);

        storage.insert(e, position(1.0, 2.0));

        assert_eq!(storage.get(e), Some(&Position { x: 1.0, y: 2.0 }));
    }

    #[test]
    fn get_nonexistent_entity_returns_none() {
        let mut storage = Storage::new();

        storage.insert(entity(0), position(1.0, 2.0));

        assert!(storage.get(entity(1)).is_none());
    }

    #[test]
    fn get_mut_modifies_component() {
        let mut storage = Storage::new();
        let e = entity(0);

        storage.insert(e, position(1.0, 2.0));

        let position = storage.get_mut(e).unwrap();
        position.x = 100.0;

        assert_eq!(storage.get(e), Some(&Position { x: 100.0, y: 2.0 }));
    }

    #[test]
    fn insert_multiple_entities() {
        let mut storage = Storage::new();

        let a = entity(0);
        let b = entity(1);
        let c = entity(2);

        storage.insert(a, position(1.0, 2.0));
        storage.insert(b, position(3.0, 4.0));
        storage.insert(c, position(5.0, 6.0));

        assert_eq!(storage.get(a), Some(&Position { x: 1.0, y: 2.0 }));

        assert_eq!(storage.get(b), Some(&Position { x: 3.0, y: 4.0 }));

        assert_eq!(storage.get(c), Some(&Position { x: 5.0, y: 6.0 }));
    }

    #[test]
    fn remove_entity() {
        let mut storage = Storage::new();
        let e = entity(0);

        storage.insert(e, position(1.0, 2.0));

        storage.remove_entity(e);

        assert!(storage.get(e).is_none());
    }

    #[test]
    fn remove_nonexistent_entity_does_nothing() {
        let mut storage = Storage::new();

        storage.insert(entity(0), position(1.0, 2.0));

        storage.remove_entity(entity(1));

        assert_eq!(storage.get(entity(0)), Some(&Position { x: 1.0, y: 2.0 }));
    }

    #[test]
    fn swap_remove_preserves_last_entity() {
        let mut storage = Storage::new();

        let a = entity(0);
        let b = entity(1);
        let c = entity(2);

        storage.insert(a, position(1.0, 2.0));
        storage.insert(b, position(3.0, 4.0));
        storage.insert(c, position(5.0, 6.0));

        storage.remove_entity(b);

        assert_eq!(storage.get(a), Some(&Position { x: 1.0, y: 2.0 }));

        assert_eq!(storage.get(c), Some(&Position { x: 5.0, y: 6.0 }));

        assert!(storage.get(b).is_none());
    }

    #[test]
    fn swap_remove_updates_sparse_index() {
        let mut storage = Storage::new();

        let a = entity(0);
        let b = entity(1);
        let c = entity(2);

        storage.insert(a, position(1.0, 2.0));
        storage.insert(b, position(3.0, 4.0));
        storage.insert(c, position(5.0, 6.0));

        storage.remove_entity(b);

        // C was moved into B's dense slot.
        // Its sparse index must therefore be updated.
        assert_eq!(storage.get(c), Some(&Position { x: 5.0, y: 6.0 }));
    }

    #[test]
    fn remove_first_entity() {
        let mut storage = Storage::new();

        let a = entity(0);
        let b = entity(1);
        let c = entity(2);

        storage.insert(a, position(1.0, 2.0));
        storage.insert(b, position(3.0, 4.0));
        storage.insert(c, position(5.0, 6.0));

        storage.remove_entity(a);

        assert!(storage.get(a).is_none());

        assert_eq!(storage.get(b), Some(&Position { x: 3.0, y: 4.0 }));

        assert_eq!(storage.get(c), Some(&Position { x: 5.0, y: 6.0 }));
    }

    #[test]
    fn remove_last_entity() {
        let mut storage = Storage::new();

        let a = entity(0);
        let b = entity(1);
        let c = entity(2);

        storage.insert(a, position(1.0, 2.0));
        storage.insert(b, position(3.0, 4.0));
        storage.insert(c, position(5.0, 6.0));

        storage.remove_entity(c);

        assert!(storage.get(c).is_none());

        assert_eq!(storage.get(a), Some(&Position { x: 1.0, y: 2.0 }));

        assert_eq!(storage.get(b), Some(&Position { x: 3.0, y: 4.0 }));
    }

    #[test]
    fn remove_all_entities() {
        let mut storage = Storage::new();

        for i in 0..100 {
            storage.insert(entity(i), position(i as f32, i as f32));
        }

        for i in 0..100 {
            storage.remove_entity(entity(i));
        }

        for i in 0..100 {
            assert!(storage.get(entity(i)).is_none());
        }
    }

    #[test]
    fn generation_is_part_of_entity_identity() {
        let mut storage = Storage::new();

        let old = EntityId::new(0, 0);
        let new = EntityId::new(0, 1);

        storage.insert(old, position(1.0, 2.0));

        assert!(storage.get(old).is_some());
        assert!(storage.get(new).is_none());
    }

    #[test]
    fn remove_then_reinsert() {
        let mut storage = Storage::new();

        let e = entity(0);

        storage.insert(e, position(1.0, 2.0));
        storage.remove_entity(e);

        assert!(storage.get(e).is_none());

        storage.insert(e, position(10.0, 20.0));

        assert_eq!(storage.get(e), Some(&Position { x: 10.0, y: 20.0 }));
    }
    #[test]
    fn insert_overwrites_existing_component() {
        let mut storage = Storage::new();

        let e = entity(0);

        storage.insert(e, position(1.0, 2.0));
        storage.insert(e, position(10.0, 20.0));

        assert_eq!(storage.get(e), Some(&Position { x: 10.0, y: 20.0 }));
    }
}
