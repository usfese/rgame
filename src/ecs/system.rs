//! ECS 系统的查询模块。
//!
//! 包含 [`QueryData`]、[`Query`] 与 [`QueryIter`]。
//!
//! 目前的 [`Query`] 只支持只读查询（`&T`）。[`Query`] 内部只持有一个 `&World`，
//! 在这样的前提下无法安全地交出 `&mut T`（多个 `&mut` 会违反借用规则，
//! 强转则属于未定义行为），因此可变访问暂时仍通过 [`World::get_mut`] 完成。

use std::marker::PhantomData;

use crate::ecs::{
    component::{Component, ErasedStorage},
    entity::EntityId,
    world::World,
};

/// 描述一次查询要取出的数据。
///
/// 目前的实现者：
/// - `&T`：取出实体上组件 `T` 的共享引用。
/// - 元组，如 `(&Position, &Velocity)`：要求实体同时拥有元组中的全部组件。
///
/// [`QueryData::Item`] 是带生命周期参数的 GAT：`Item<'w>` 中的 `'w` 只与 [`World`] 的
/// 借用绑定，与 `Self` 自身的生命周期无关。因此 `Query<(&'a T, &'b U)>` 里的 `'a`、`'b`
/// 可以随便填（甚至是 `'static`），取出来的数据依然只在 `World` 被借用期间有效。
pub trait QueryData {
    /// 从单个实体上取出的数据。
    type Item<'w>;

    /// 取出 `entity` 上的数据；只要缺少任意一个组件就返回 [`None`]。
    fn get<'w>(world: &'w World, entity: EntityId) -> Option<Self::Item<'w>>;

    /// 本次查询会用到的所有 Storage。
    ///
    /// 用来挑选遍历时的驱动 Storage，同时也用来检查同一个组件类型是否被重复查询。
    fn storages<'w>(world: &'w World) -> Vec<&'w dyn ErasedStorage>;
}

/// 组件 `T` 的共享引用查询。
impl<T: Component> QueryData for &T {
    type Item<'w> = &'w T;

    #[inline]
    fn get<'w>(world: &'w World, entity: EntityId) -> Option<Self::Item<'w>> {
        world.get::<T>(entity)
    }

    fn storages<'w>(world: &'w World) -> Vec<&'w dyn ErasedStorage> {
        // 该类型的 Storage 还不存在（从未插入过任何该组件）时返回空列表，
        // 此时查询既取不到数据，也不会遍历任何实体。
        match world.get_components().get::<T>() {
            Some(storage) => vec![storage as &dyn ErasedStorage],
            None => Vec::new(),
        }
    }
}

macro_rules! impl_query_data {
    ($($T:ident),+) => {
        impl<$($T: QueryData),+> QueryData for ($($T,)+) {
            type Item<'w> = ($($T::Item<'w>,)+);

            fn get<'w>(
                world: &'w World,
                entity: EntityId,
            ) -> Option<Self::Item<'w>> {
                Some((
                    $($T::get(world, entity)?,)+
                ))
            }

            fn storages<'w>(
                world: &'w World,
            ) -> Vec<&'w dyn ErasedStorage> {
                let mut storages = Vec::new();

                $(
                    storages.extend($T::storages(world));
                )+

                storages
            }
        }
    };
}

impl_query_data!(A);
impl_query_data!(A, B);
impl_query_data!(A, B, C);
impl_query_data!(A, B, C, D);
impl_query_data!(A, B, C, D, E);
impl_query_data!(A, B, C, D, E, F);
impl_query_data!(A, B, C, D, E, F, G);
impl_query_data!(A, B, C, D, E, F, G, H);
impl_query_data!(A, B, C, D, E, F, G, H, I);
impl_query_data!(A, B, C, D, E, F, G, H, I, J);
impl_query_data!(A, B, C, D, E, F, G, H, I, J, K);
impl_query_data!(A, B, C, D, E, F, G, H, I, J, K, L);

/// 一次查询。
///
/// 由 [`World::query`] 创建，只借用 `World` 的共享引用，
/// 因此可以同时存在多个 `Query`（只读查询之间不会冲突）。
pub struct Query<'w, D: QueryData> {
    world: &'w World,
    _marker: PhantomData<D>,
}

/// [`Query::iter`] 返回的迭代器。
///
/// 内部持有一个驱动 Storage 的实体列表切片，遍历时用其余 Storage 做过滤。
pub struct QueryIter<'w, D: QueryData> {
    world: &'w World,
    entities: std::slice::Iter<'w, EntityId>,
    _marker: PhantomData<D>,
}

impl<'w, D: QueryData> Query<'w, D> {
    pub(crate) fn new(world: &'w World) -> Self {
        Self {
            world,
            _marker: PhantomData,
        }
    }

    /// 取出实体 `entity` 上的数据，缺少任意一个查询组件时返回 [`None`]。
    ///
    /// 返回值的生命周期是 `'w`（`World` 的借用）而不是 `&self` 的借用，
    /// 因为查询本身只持有共享引用，可以复制出去。
    pub fn get(&self, entity: EntityId) -> Option<D::Item<'w>> {
        D::get(self.world, entity)
    }

    /// 遍历所有满足查询条件的实体。
    ///
    /// 结果中实体的顺序取决于驱动 Storage 内部的稠密数组顺序，不保证稳定。
    pub fn iter(&self) -> QueryIter<'w, D> {
        QueryIter::new(self.world)
    }
}

impl<'w, D: QueryData> QueryIter<'w, D> {
    pub(crate) fn new(world: &'w World) -> Self {
        // 选出实体最少的 Storage 作为驱动：遍历它，再用其它 Storage 过滤。
        // 全部 Storage 都不存在时（组件从未被插入过）驱动列表为空，迭代结果也为空。
        let entities: &'w [EntityId] = D::storages(world)
            .into_iter()
            .min_by_key(|storage| storage.entities().len())
            .map_or(&[], |storage| storage.entities());

        Self {
            world,
            entities: entities.iter(),
            _marker: PhantomData,
        }
    }
}

impl<'w, D: QueryData> Iterator for QueryIter<'w, D> {
    type Item = D::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        // 驱动 Storage 里可能存在缺少其它查询组件的实体，跳过它们。
        while let Some(&entity) = self.entities.next() {
            if let Some(item) = D::get(self.world, entity) {
                return Some(item);
            }
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // 驱动 Storage 的剩余实体数是结果数的上界。
        (0, Some(self.entities.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::component::Component;

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

    #[test]
    fn query_get_returns_borrowed_component() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        assert_eq!(
            world.query::<&Position>().get(entity),
            Some(&Position { x: 1.0, y: 2.0 })
        );
    }

    #[test]
    fn query_get_missing_component_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();

        assert_eq!(world.query::<&Position>().get(entity), None);
    }

    #[test]
    fn query_get_on_dead_entity_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        world.despawn(entity);

        assert_eq!(world.query::<&Position>().get(entity), None);
    }

    #[test]
    fn query_iter_yields_all_matching_entities() {
        let mut world = World::new();

        let entities: Vec<_> = (0..10).map(|_| world.spawn()).collect();

        for &entity in &entities {
            world.insert(entity, Position { x: 1.0, y: 2.0 });
        }

        assert_eq!(world.query::<&Position>().iter().count(), 10);
    }

    #[test]
    fn query_iter_skips_entities_missing_a_component() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();

        world.insert(a, Position { x: 1.0, y: 1.0 });
        world.insert(b, Position { x: 2.0, y: 2.0 });
        world.insert(c, Position { x: 3.0, y: 3.0 });

        // 只有 b 同时拥有 Velocity。
        world.insert(b, Velocity { x: 0.5, y: 0.25 });

        let found: Vec<_> = world
            .query::<(&Position, &Velocity)>()
            .iter()
            .map(|(position, velocity)| (position.x + velocity.x, position.y + velocity.y))
            .collect();

        assert_eq!(found, vec![(2.5, 2.25)]);
    }

    #[test]
    fn query_iter_works_with_uneven_storages() {
        let mut world = World::new();

        // Position 很多、Velocity 很少，无论以哪个 Storage 驱动都必须结果正确。
        let entities: Vec<_> = (0..64).map(|_| world.spawn()).collect();

        for &entity in &entities {
            world.insert(entity, Position { x: 0.0, y: 0.0 });
        }

        world.insert(entities[10], Velocity { x: 1.0, y: 2.0 });
        world.insert(entities[40], Velocity { x: 3.0, y: 4.0 });

        let mut sums: Vec<_> = world
            .query::<(&Position, &Velocity)>()
            .iter()
            .map(|(_, velocity)| velocity.x + velocity.y)
            .collect();

        sums.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert_eq!(sums, vec![3.0, 7.0]);
    }

    #[test]
    fn query_iter_skips_despawned_entities() {
        let mut world = World::new();

        let entities: Vec<_> = (0..5).map(|_| world.spawn()).collect();

        for &entity in &entities {
            world.insert(entity, Position { x: 1.0, y: 2.0 });
        }

        world.despawn(entities[2]);

        assert_eq!(world.query::<&Position>().iter().count(), 4);
    }

    #[test]
    fn query_iter_is_empty_when_component_was_never_inserted() {
        let mut world = World::new();

        world.spawn();
        world.spawn();

        assert_eq!(world.query::<&Position>().iter().count(), 0);
    }

    #[test]
    fn query_iter_three_components() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();

        world.insert(a, Position { x: 1.0, y: 1.0 });
        world.insert(a, Velocity { x: 2.0, y: 2.0 });

        world.insert(b, Position { x: 3.0, y: 3.0 });
        world.insert(b, Velocity { x: 4.0, y: 4.0 });

        let count = world
            .query::<(&Position, &Velocity, &Position)>()
            .iter()
            .count();

        assert_eq!(count, 2);
    }

    #[test]
    fn query_iter_does_not_yield_duplicates() {
        let mut world = World::new();

        let entities: Vec<_> = (0..8).map(|_| world.spawn()).collect();

        for &entity in &entities {
            world.insert(entity, Position { x: 1.0, y: 2.0 });
            world.insert(entity, Velocity { x: 3.0, y: 4.0 });
        }

        let slots: Vec<_> = world
            .query::<(&Position, &Velocity)>()
            .iter()
            .map(|(_, _)| ())
            .collect();

        assert_eq!(slots.len(), entities.len());
    }

    #[test]
    fn multiple_queries_can_coexist() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });
        world.insert(entity, Velocity { x: 3.0, y: 4.0 });

        // 只读查询只借用 &World，因此可以同时存在。
        let positions = world.query::<&Position>();
        let velocities = world.query::<&Velocity>();

        assert_eq!(positions.get(entity), Some(&Position { x: 1.0, y: 2.0 }));

        assert_eq!(velocities.get(entity), Some(&Velocity { x: 3.0, y: 4.0 }));
    }

    #[test]
    fn borrowed_query_item_outlives_query() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        // 取出的引用绑定在 World 的借用上，而不是 Query 自身的借用上。
        let position = world.query::<&Position>().get(entity).unwrap();

        assert_eq!(position.x, 1.0);
        assert_eq!(position.y, 2.0);
    }
}
