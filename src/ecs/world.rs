//! ECS系统的世界模块。
//!
//! 包含[`World`]。

use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::ecs::{
    component::*,
    entity::{self, *},
    system::{Query, QueryData, QueryDataMut, QueryMut},
};

pub(crate) struct ComponentRegistry {
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            storages: HashMap::new(),
        }
    }

    /// 获取指定Component类型的Storage引用。
    pub fn get<T: Component>(&self) -> Option<&Storage<T>> {
        self.storages
            .get(&TypeId::of::<T>())
            .and_then(|storage| storage.as_any().downcast_ref::<Storage<T>>())
    }

    /// 获取指定Component类型的Storage可变引用。
    pub fn get_mut<T: Component>(&mut self) -> Option<&mut Storage<T>> {
        self.storages
            .get_mut(&TypeId::of::<T>())
            .and_then(|storage| storage.as_mut_any().downcast_mut::<Storage<T>>())
    }

    /// 获取指定类型的 Storage，不存在则创建。
    pub fn get_or_insert<T: Component>(&mut self) -> &mut Storage<T> {
        let type_id = TypeId::of::<T>();

        self.storages
            .entry(type_id)
            .or_insert_with(|| Box::new(Storage::<T>::new()))
            .as_mut_any()
            .downcast_mut::<Storage<T>>()
            .unwrap()
    }

    /// 一次性获取多个不同组件类型的 Storage 可变引用。
    ///
    /// `type_ids` 中不能有重复项；调用方需要先检查，否则
    /// [`std::collections::HashMap::get_disjoint_mut`] 会 panic。
    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        type_ids: [TypeId; N],
    ) -> [Option<&mut dyn ErasedStorage>; N] {
        let keys: [&TypeId; N] = std::array::from_fn(|index| &type_ids[index]);

        self.storages
            .get_disjoint_mut(keys)
            .map(|storage| storage.map(|storage| &mut **storage))
    }

    pub fn contains<T: Component>(&self) -> bool {
        self.storages.contains_key(&TypeId::of::<T>())
    }

    /// 移除指定类型的 Storage，并返回它
    /// 会删除该类型的所有组件
    pub fn remove<T: Component>(&mut self) -> Option<Storage<T>> {
        self.storages
            .remove(&TypeId::of::<T>())
            .map(|storage| *storage.into_any().downcast::<Storage<T>>().unwrap())
    }

    /// 从所有 Storage 中移除指定实体的组件。
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

/// 游戏世界结构体。
/// 负责管理[`EntityId`]和[`Component`]。
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

    /// 创建新的实体，实体以[`EntityId`]形式返回。
    pub fn spawn(&mut self) -> EntityId {
        self.entities.spawn()
    }

    /// 销毁世界中id为`entity`的实体，并删除有关的所有组件
    pub fn despawn(&mut self, entity: EntityId) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }
        self.components.remove_entity(entity);
        self.entities.destroy(entity);
        true
    }

    /// 为世界中的`entity`实体添加组件`component`。如果实体已拥有该类型的组件，则替换原有组件。
    ///
    /// # Note
    /// 对于给定的组件类型，一个实体至多拥有一个组件实例。
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

    /// 删除实体的一个组件
    pub fn remove_component<T: Component>(&mut self, entity: EntityId) -> Option<T> {
        if !self.is_alive(entity) {
            return None;
        }
        let storage = self.components.get_mut::<T>()?;

        storage.remove(entity)
    }

    pub(crate) fn get_components(&self) -> &ComponentRegistry {
        &self.components
    }

    pub fn query<D: QueryData>(&self) -> Query<'_, D> {
        Query::new(self)
    }

    pub(crate) fn components_mut(&mut self) -> &mut ComponentRegistry {
        &mut self.components
    }

    /// 创建一次可变查询。
    ///
    /// 查询持有 `&mut World`，因此查询存活期间不能修改世界结构
    /// （spawn/despawn/insert/remove）。
    pub(crate) fn query_mut<D: QueryDataMut>(&mut self) -> QueryMut<'_, D> {
        QueryMut::new(self)
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
