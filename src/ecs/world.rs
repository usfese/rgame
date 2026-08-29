use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::ecs::{
    component::*,
    entity::{self, *},
};

pub struct ComponentRegistry {
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            storages: HashMap::new(),
        }
    }

    pub fn get<T: Component>(&self) -> Option<&Storage<T>> {
        self.storages
            .get(&TypeId::of::<T>())
            .and_then(|storage| storage.as_any().downcast_ref::<Storage<T>>())
    }

    pub fn get_mut<T: Component>(&mut self) -> Option<&mut Storage<T>> {
        self.storages
            .get_mut(&TypeId::of::<T>())
            .and_then(|storage| storage.as_mut_any().downcast_mut::<Storage<T>>())
    }

    pub fn get_or_insert<T: Component>(&mut self) -> &mut Storage<T> {
        let type_id = TypeId::of::<T>();

        self.storages
            .entry(type_id)
            .or_insert_with(|| Box::new(Storage::<T>::new()))
            .as_mut_any()
            .downcast_mut::<Storage<T>>()
            .unwrap()
    }

    pub fn contains<T: Component>(&self) -> bool {
        self.storages.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: Component>(&mut self) -> Option<Storage<T>> {
        self.storages
            .remove(&TypeId::of::<T>())
            .map(|storage| *storage.into_any().downcast::<Storage<T>>().unwrap())
    }
    pub fn remove_entity(&mut self, entity: EntityId) {
        for storage in self.storages.values_mut() {
            storage.remove_entity(entity);
        }
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct World {
    entities: EntityManager,
    components: ComponentRegistry,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: EntityManager::new(),
            components: ComponentRegistry::new(),
        }
    }

    pub fn spawn(&mut self) -> EntityId {
        self.entities.spawn()
    }

    pub fn despawn(&mut self, entity: EntityId) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        self.components.remove_entity(entity);
        self.entities.destroy(entity);
        true
    }

    pub fn insert<T: Component>(&mut self, entity: EntityId, component: T) {}
}
