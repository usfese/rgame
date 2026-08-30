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

    pub fn insert<T: Component>(&mut self, entity: EntityId, component: T) {
        if self.entities.is_alive(entity) {
            self.components
                .get_or_insert::<T>()
                .insert(entity, component);
        }
    }

    #[inline]
    pub fn entity_count(&self) -> usize {
        self.entities.entity_count()
    }

    #[inline]
    pub fn is_alive(&self, entity: EntityId) -> bool {
        self.entities.is_alive(entity)
    }

    pub fn get<T: Component>(&self, entity: EntityId) -> Option<&T> {
        if !self.is_alive(entity) {
            return None;
        }

        self.components.get::<T>()?.get(entity)
    }

    pub fn get_mut<T: Component>(&mut self, entity: EntityId) -> Option<&mut T> {
        if !self.is_alive(entity) {
            return None;
        }

        self.components.get_mut()?.get_mut(entity)
    }

    pub fn remove_component<T: Component>(&mut self, entity: EntityId) -> Option<T> {
        if !self.is_alive(entity) {
            return None;
        }
        let storage = self.components.get_mut::<T>()?;

        storage.remove(entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::component::Component;
    use crate::ecs::entity::EntityId;

    #[derive(Debug, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    impl Component for Position {}

    #[derive(Debug, PartialEq)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    impl Component for Velocity {}

    // ------------------------------------------------------------
    // spawn / despawn
    // ------------------------------------------------------------

    #[test]
    fn spawn_creates_entity() {
        let mut world = World::new();

        let entity = world.spawn();

        assert!(world.is_alive(entity));
    }

    #[test]
    fn spawned_entities_are_distinct() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();

        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(b, c);

        assert!(world.is_alive(a));
        assert!(world.is_alive(b));
        assert!(world.is_alive(c));
    }

    #[test]
    fn entity_count_increases_on_spawn() {
        let mut world = World::new();

        assert_eq!(world.entity_count(), 0);

        let a = world.spawn();
        assert_eq!(world.entity_count(), 1);

        let b = world.spawn();
        assert_eq!(world.entity_count(), 2);

        let c = world.spawn();
        assert_eq!(world.entity_count(), 3);

        assert!(world.is_alive(a));
        assert!(world.is_alive(b));
        assert!(world.is_alive(c));
    }

    #[test]
    fn entity_count_decreases_on_despawn() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();

        assert_eq!(world.entity_count(), 2);

        world.despawn(a);
        assert_eq!(world.entity_count(), 1);

        world.despawn(b);
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn despawn_makes_entity_dead() {
        let mut world = World::new();

        let entity = world.spawn();

        assert!(world.is_alive(entity));

        world.despawn(entity);

        assert!(!world.is_alive(entity));
    }

    #[test]
    fn despawning_dead_entity_does_nothing() {
        let mut world = World::new();

        let entity = world.spawn();

        world.despawn(entity);
        assert_eq!(world.entity_count(), 0);

        world.despawn(entity);
        assert_eq!(world.entity_count(), 0);
    }

    // ------------------------------------------------------------
    // generation / reuse
    // ------------------------------------------------------------

    #[test]
    fn reused_slot_gets_new_generation() {
        let mut world = World::new();

        let old = world.spawn();
        world.despawn(old);

        let new = world.spawn();

        assert_ne!(old, new);
        assert_eq!(old.slot(), new.slot());
        assert_ne!(old.generation(), new.generation());

        assert!(!world.is_alive(old));
        assert!(world.is_alive(new));
    }

    #[test]
    fn old_entity_cannot_access_reused_entity() {
        let mut world = World::new();

        let old = world.spawn();
        world.insert(old, Position { x: 1.0, y: 2.0 });

        world.despawn(old);

        let new = world.spawn();

        // The old ID must never become valid again.
        assert!(!world.is_alive(old));

        assert_eq!(world.get::<Position>(old), None);

        world.insert(new, Position { x: 10.0, y: 20.0 });

        assert_eq!(
            world.get::<Position>(new),
            Some(&Position { x: 10.0, y: 20.0 })
        );

        assert_eq!(world.get::<Position>(old), None);
    }

    #[test]
    fn multiple_entities_can_be_reused() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();

        world.despawn(a);
        world.despawn(b);
        world.despawn(c);

        let a2 = world.spawn();
        let b2 = world.spawn();
        let c2 = world.spawn();

        assert_ne!(a, a2);
        assert_ne!(b, b2);
        assert_ne!(c, c2);

        assert_eq!(a.slot(), a2.slot());
        assert_eq!(b.slot(), b2.slot());
        assert_eq!(c.slot(), c2.slot());

        assert!(world.is_alive(a2));
        assert!(world.is_alive(b2));
        assert!(world.is_alive(c2));
    }

    // ------------------------------------------------------------
    // insert / get
    // ------------------------------------------------------------

    #[test]
    fn insert_and_get_component() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 10.0, y: 20.0 });

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 10.0, y: 20.0 })
        );
    }

    #[test]
    fn get_missing_component_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();

        assert_eq!(world.get::<Position>(entity), None);
    }

    #[test]
    fn insert_multiple_components() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.insert(entity, Velocity { x: 3.0, y: 4.0 });

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 1.0, y: 2.0 })
        );

        assert_eq!(
            world.get::<Velocity>(entity),
            Some(&Velocity { x: 3.0, y: 4.0 })
        );
    }

    #[test]
    fn insert_overwrites_component() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.insert(entity, Position { x: 10.0, y: 20.0 });

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 10.0, y: 20.0 })
        );
    }

    #[test]
    fn insert_does_not_affect_other_entities() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();

        world.insert(a, Position { x: 1.0, y: 2.0 });

        world.insert(b, Position { x: 3.0, y: 4.0 });

        assert_eq!(world.get::<Position>(a), Some(&Position { x: 1.0, y: 2.0 }));

        assert_eq!(world.get::<Position>(b), Some(&Position { x: 3.0, y: 4.0 }));
    }

    // ------------------------------------------------------------
    // get_mut
    // ------------------------------------------------------------

    #[test]
    fn get_mut_modifies_component() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        let position = world.get_mut::<Position>(entity).unwrap();

        position.x = 100.0;
        position.y = 200.0;

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 100.0, y: 200.0 })
        );
    }

    #[test]
    fn get_mut_missing_component_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();

        assert!(world.get_mut::<Position>(entity).is_none());
    }

    // ------------------------------------------------------------
    // remove
    // ------------------------------------------------------------

    #[test]
    fn remove_component() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 10.0, y: 20.0 });

        assert!(world.get::<Position>(entity).is_some());

        let removed = world.remove_component::<Position>(entity);

        assert_eq!(removed, Some(Position { x: 10.0, y: 20.0 }));

        assert_eq!(world.get::<Position>(entity), None);
    }

    #[test]
    fn remove_missing_component_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();

        assert_eq!(world.remove_component::<Position>(entity), None);
    }

    #[test]
    fn remove_one_component_preserves_others() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.insert(entity, Velocity { x: 3.0, y: 4.0 });

        let removed = world.remove_component::<Position>(entity);

        assert_eq!(removed, Some(Position { x: 1.0, y: 2.0 }));

        assert_eq!(world.get::<Position>(entity), None);

        assert_eq!(
            world.get::<Velocity>(entity),
            Some(&Velocity { x: 3.0, y: 4.0 })
        );
    }

    // ------------------------------------------------------------
    // despawn + components
    // ------------------------------------------------------------

    #[test]
    fn despawn_removes_all_components() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.insert(entity, Velocity { x: 3.0, y: 4.0 });

        world.despawn(entity);

        assert!(!world.is_alive(entity));
        assert_eq!(world.get::<Position>(entity), None);
        assert_eq!(world.get::<Velocity>(entity), None);
    }

    #[test]
    fn despawn_does_not_affect_other_entities() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();

        world.insert(a, Position { x: 1.0, y: 2.0 });

        world.insert(b, Position { x: 3.0, y: 4.0 });

        world.despawn(a);

        assert_eq!(world.get::<Position>(a), None);

        assert_eq!(world.get::<Position>(b), Some(&Position { x: 3.0, y: 4.0 }));
    }

    #[test]
    fn components_can_be_inserted_after_entity_reuse() {
        let mut world = World::new();

        let old = world.spawn();

        world.insert(old, Position { x: 1.0, y: 2.0 });

        world.despawn(old);

        let new = world.spawn();

        world.insert(new, Position { x: 10.0, y: 20.0 });

        assert_eq!(
            world.get::<Position>(new),
            Some(&Position { x: 10.0, y: 20.0 })
        );

        assert_eq!(world.get::<Position>(old), None);
    }

    #[test]
    fn despawn_then_spawn_does_not_keep_old_components() {
        let mut world = World::new();

        let old = world.spawn();

        world.insert(old, Position { x: 1.0, y: 2.0 });

        world.despawn(old);

        let new = world.spawn();

        assert_eq!(world.get::<Position>(new), None);
    }

    // ------------------------------------------------------------
    // dead entity operations
    // ------------------------------------------------------------

    #[test]
    fn get_on_dead_entity_returns_none() {
        let mut world = World::new();

        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.despawn(entity);

        assert_eq!(world.get::<Position>(entity), None);
    }

    #[test]
    fn get_mut_on_dead_entity_returns_none() {
        let mut world = World::new();

        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.despawn(entity);

        assert!(world.get_mut::<Position>(entity).is_none());
    }

    #[test]
    fn remove_from_dead_entity_returns_none() {
        let mut world = World::new();

        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.despawn(entity);

        assert_eq!(world.remove_component::<Position>(entity), None);
    }
}
