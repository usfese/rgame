//! ECS 系统的查询模块。
//!
//! 包含：
//! - 只读查询：`QueryData`、`Query`、`QueryIter`、`QueryIterWithEntities`；
//! - 可变查询：`QueryDataMut`、`QueryMut`。
//!
//! 只读 [`Query`] 只借用 `&World` 并持有从中借出的 Storage 引用，
//! 可以同时存在多个只读查询。
//! 可变查询独占 `&mut World`，构造时一次性借出所需的 Storage，
//! 支持 `&mut T`、`&T` 以及元组（如 `(&Velocity, &mut Position)`）。
//! 同一个可变查询中不允许重复出现同一组件类型；只读查询允许重复类型。

use std::any::TypeId;

use crate::ecs::{
    access::{self, Access},
    component::{Component, ErasedStorage, Storage},
    entity::EntityId,
    world::{ComponentRegistry, World},
};

/// 描述一次查询要取出的数据。
///
/// 目前的实现者：
/// - `&T`：取出实体上组件 `T` 的共享引用。
/// - 元组，如 `(&Position, &Velocity)`：要求实体同时拥有元组中的全部组件。
///
/// 只读查询允许同一个组件类型在元组中重复出现，此时会得到同一个组件的多个
/// 共享引用（可变查询则会拒绝重复类型）。
///
/// [`QueryData::Item`] 是带生命周期参数的 GAT：`Item<'w>` 中的 `'w` 只与 [`World`] 的
/// 借用绑定，与 `Self` 自身的生命周期无关。因此 `Query<(&'a T, &'b U)>` 里的 `'a`、`'b`
/// 可以随便填（甚至是 `'static`），取出来的数据依然只在 `World` 被借用期间有效。
pub(crate) trait QueryData {
    /// 从单个实体上取出的数据。
    type Item<'w>;

    /// 查询构造时一次性借出的 Storage，或 Storage 元组。
    ///
    /// 只读查询的 Fetch 只包含共享引用，因此要求 [`Copy`]：
    /// [`Query`] 可以把它复制给任意多个 [`QueryIter`]。
    type Fetch<'w>: Copy;

    /// 借出本次查询需要的全部 Storage；任一组件类型尚无 Storage 时返回 [`None`]。
    fn fetch<'w>(world: &'w World) -> Option<Self::Fetch<'w>>;

    /// 从已借出的 Storage 取出 `entity` 上的数据；缺少任意一个组件时返回 [`None`]。
    fn get<'w>(fetch: Self::Fetch<'w>, entity: EntityId) -> Option<Self::Item<'w>>;

    /// 本次查询用于驱动遍历的实体列表。
    ///
    /// 返回切片的生命周期直接绑定到 [`World`] 的借用，而不是 `fetch` 自身的借用。
    fn entities<'w>(fetch: Self::Fetch<'w>) -> &'w [EntityId];
}

/// 组件 `T` 的共享引用查询。
impl<T: Component> QueryData for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = &'w Storage<T>;

    #[inline]
    fn fetch<'w>(world: &'w World) -> Option<Self::Fetch<'w>> {
        world.get_components().get::<T>()
    }

    #[inline]
    fn get<'w>(fetch: Self::Fetch<'w>, entity: EntityId) -> Option<Self::Item<'w>> {
        fetch.get(entity)
    }

    #[inline]
    fn entities<'w>(fetch: Self::Fetch<'w>) -> &'w [EntityId] {
        fetch.entities()
    }
}

/// 描述一次可变查询要取出的数据。
///
/// 与只读的 [`QueryData`] 不同，可变查询在构造时独占 `&mut World`，
/// 并通过 [`QueryDataMut::Fetch`] 一次性借出互不重叠的 Storage。
///
/// 目前的实现者：
/// - `&mut T`：可变引用；
/// - `&T`：在可变查询中只读；
/// - 元组，如 `(&Velocity, &mut Position)`：要求实体同时拥有元组中的全部组件。
///
/// 同一个查询中不能重复出现同一个组件类型（包括 `&T` 与 `&mut T` 混用），
/// 否则构造查询时会 panic。
pub(crate) trait QueryDataMut {
    /// 从单个实体上取出的数据。
    type Item<'w>;

    /// 查询构造时一次性借出的 Storage，或 Storage 元组。
    type Fetch<'w>;

    /// 该查询访问的组件类型。
    fn type_id() -> TypeId;

    /// 从组件注册表借出该查询需要的 Storage。
    fn fetch<'w>(components: &'w mut ComponentRegistry) -> Option<Self::Fetch<'w>>;

    /// 从调度器预先借出的单个 Storage 构造本查询的 Fetch。
    ///
    /// 供元组查询使用；叶子实现（`&T`、`&mut T`）直接包装对应的 Storage。
    fn fetch_one<'w>(storage: &'w mut dyn ErasedStorage) -> Option<Self::Fetch<'w>>;

    /// 取出 `entity` 上的数据；缺少任意一个组件时返回 [`None`]。
    fn get<'a, 'w: 'a>(fetch: &'a mut Self::Fetch<'w>, entity: EntityId) -> Option<Self::Item<'a>>;

    /// 本次查询用于驱动遍历的实体列表。
    fn entities<'a, 'w: 'a>(fetch: &'a Self::Fetch<'w>) -> &'a [EntityId];

    fn access(access: &mut Access);
}

impl<T: Component> QueryDataMut for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = &'w mut Storage<T>;

    #[inline]
    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    #[inline]
    fn fetch<'w>(components: &'w mut ComponentRegistry) -> Option<Self::Fetch<'w>> {
        components.get_mut::<T>()
    }

    #[inline]
    fn fetch_one<'w>(storage: &'w mut dyn ErasedStorage) -> Option<Self::Fetch<'w>> {
        storage.as_mut_any().downcast_mut::<Storage<T>>()
    }

    #[inline]
    fn get<'a, 'w: 'a>(fetch: &'a mut Self::Fetch<'w>, entity: EntityId) -> Option<Self::Item<'a>> {
        (*fetch).get_mut(entity)
    }

    #[inline]
    fn entities<'a, 'w: 'a>(fetch: &'a Self::Fetch<'w>) -> &'a [EntityId] {
        (**fetch).entities()
    }

    #[inline]
    fn access(access: &mut Access) {
        access.add_read(TypeId::of::<T>());
    }
}

impl<T: Component> QueryDataMut for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = &'w mut Storage<T>;

    #[inline]
    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    #[inline]
    fn fetch<'w>(components: &'w mut ComponentRegistry) -> Option<Self::Fetch<'w>> {
        components.get_mut::<T>()
    }

    #[inline]
    fn fetch_one<'w>(storage: &'w mut dyn ErasedStorage) -> Option<Self::Fetch<'w>> {
        storage.as_mut_any().downcast_mut::<Storage<T>>()
    }

    #[inline]
    fn get<'a, 'w: 'a>(fetch: &'a mut Self::Fetch<'w>, entity: EntityId) -> Option<Self::Item<'a>> {
        (*fetch).get(entity)
    }

    #[inline]
    fn entities<'a, 'w: 'a>(fetch: &'a Self::Fetch<'w>) -> &'a [EntityId] {
        (**fetch).entities()
    }

    #[inline]
    fn access(access: &mut Access) {
        access.add_write(TypeId::of::<T>());
    }
}

macro_rules! impl_query_data {
    ($($T:ident => $index:tt),+) => {
        impl<$($T: QueryData),+> QueryData for ($($T,)+) {
            type Item<'w> = ($($T::Item<'w>,)+);
            type Fetch<'w> = ($($T::Fetch<'w>,)+);

            fn fetch<'w>(world: &'w World) -> Option<Self::Fetch<'w>> {
                Some(($($T::fetch(world)?,)+))
            }

            fn get<'w>(
                fetch: Self::Fetch<'w>,
                entity: EntityId,
            ) -> Option<Self::Item<'w>> {
                Some((
                    $($T::get(fetch.$index, entity)?,)+
                ))
            }

            fn entities<'w>(
                fetch: Self::Fetch<'w>,
            ) -> &'w [EntityId] {
                let mut result: Option<&'w [EntityId]> = None;

                $(
                    let entities = $T::entities(fetch.$index);

                    if result.map_or(true, |current| entities.len() < current.len()) {
                        result = Some(entities);
                    }
                )+

                result.unwrap_or(&[])
            }
        }
    };
}

impl_query_data!(A => 0);
impl_query_data!(A => 0, B => 1);
impl_query_data!(A => 0, B => 1, C => 2);
impl_query_data!(A => 0, B => 1, C => 2, D => 3);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9, K => 10);
impl_query_data!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9, K => 10, L => 11);

/// 检查可变查询中没有重复的组件类型。
fn assert_distinct(type_ids: &[TypeId]) {
    for (index, type_id) in type_ids.iter().enumerate() {
        assert!(
            !type_ids[index + 1..].contains(type_id),
            "mutable query contains the same component type more than once"
        );
    }
}

macro_rules! impl_query_data_mut {
    ($($T:ident => $fetch:ident => $index:tt),+) => {
        impl<$($T: QueryDataMut),+> QueryDataMut for ($($T,)+) {
            type Item<'w> = ($($T::Item<'w>,)+);
            type Fetch<'w> = ($($T::Fetch<'w>,)+);

            fn type_id() -> TypeId {
                panic!("nested mutable query tuples are not supported")
            }

            fn fetch<'w>(
                components: &'w mut ComponentRegistry,
            ) -> Option<Self::Fetch<'w>> {
                let type_ids = [$($T::type_id()),+];
                assert_distinct(&type_ids);

                let [$($fetch),+] = components.get_disjoint_mut(type_ids);

                Some(($($T::fetch_one($fetch?)?,)+))
            }

            fn fetch_one<'w>(
                _storage: &'w mut dyn ErasedStorage,
            ) -> Option<Self::Fetch<'w>> {
                panic!("nested mutable query tuples are not supported")
            }

            fn get<'a, 'w: 'a>(
                fetch: &'a mut Self::Fetch<'w>,
                entity: EntityId,
            ) -> Option<Self::Item<'a>> {
                Some(($($T::get(&mut fetch.$index, entity)?,)+))
            }

            fn entities<'a, 'w: 'a>(
                fetch: &'a Self::Fetch<'w>,
            ) -> &'a [EntityId] {
                let mut result: Option<&'a [EntityId]> = None;

                $(
                    let entities = $T::entities(&fetch.$index);

                    if result.map_or(true, |current| entities.len() < current.len()) {
                        result = Some(entities);
                    }
                )+

                result.unwrap_or(&[])
            }

            fn access(access: &mut Access) {
                $($T::access(access);)+
            }
        }
    };
}

impl_query_data_mut!(A => a => 0);
impl_query_data_mut!(A => a => 0, B => b => 1);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4, F => f => 5);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4, F => f => 5, G => g => 6);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4, F => f => 5, G => g => 6, H => h => 7);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4, F => f => 5, G => g => 6, H => h => 7, I => i => 8);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4, F => f => 5, G => g => 6, H => h => 7, I => i => 8, J => j => 9);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4, F => f => 5, G => g => 6, H => h => 7, I => i => 8, J => j => 9, K => k => 10);
impl_query_data_mut!(A => a => 0, B => b => 1, C => c => 2, D => d => 3, E => e => 4, F => f => 5, G => g => 6, H => h => 7, I => i => 8, J => j => 9, K => k => 10, L => l => 11);

/// 一次查询。
///
/// 由 [`World::query`] 创建，只借用 `World` 的共享引用，
/// 因此可以同时存在多个 `Query`（只读查询之间不会冲突）。
pub(crate) struct Query<'w, D: QueryData> {
    fetch: Option<D::Fetch<'w>>,
}

/// [`Query::iter`] 返回的迭代器。
///
/// 内部持有构造查询时借出的 Storage 和一个驱动 Storage 的实体列表切片，
/// 遍历时用其余 Storage 做过滤。
pub(crate) struct QueryIter<'w, D: QueryData> {
    fetch: Option<D::Fetch<'w>>,
    entities: std::slice::Iter<'w, EntityId>,
}

/// [`Query::iter_with_entities`] 返回的迭代器。
///
/// 与 [`QueryIter`] 相同，但同时产出实体 ID。
pub(crate) struct QueryIterWithEntities<'w, D: QueryData> {
    inner: QueryIter<'w, D>,
}

impl<'w, D: QueryData> Query<'w, D> {
    pub(crate) fn new(world: &'w World) -> Self {
        Self {
            fetch: D::fetch(world),
        }
    }

    /// 取出实体 `entity` 上的数据，缺少任意一个查询组件时返回 [`None`]。
    ///
    /// 返回值的生命周期是 `'w`（`World` 的借用）而不是 `&self` 的借用，
    /// 因为查询本身只持有共享引用，可以复制出去。
    pub fn get(&self, entity: EntityId) -> Option<D::Item<'w>> {
        D::get(self.fetch?, entity)
    }

    /// 遍历所有满足查询条件的实体。
    ///
    /// 结果中实体的顺序取决于驱动 Storage 内部的稠密数组顺序，不保证稳定。
    pub fn iter(&self) -> QueryIter<'w, D> {
        QueryIter::new(self.fetch)
    }

    /// 遍历所有满足查询条件的实体，同时产出实体 ID。
    ///
    /// 结果顺序与 [`Query::iter`] 相同。
    pub fn iter_with_entities(&self) -> QueryIterWithEntities<'w, D> {
        QueryIterWithEntities { inner: self.iter() }
    }
}

impl<'w, D: QueryData> QueryIter<'w, D> {
    pub(crate) fn new(fetch: Option<D::Fetch<'w>>) -> Self {
        // 选出实体最少的 Storage 作为驱动：遍历它，再用其它 Storage 过滤。
        // 查询涉及的任一 Storage 不存在时（该组件从未被插入过）结果必然为空。
        let entities: &'w [EntityId] = fetch.map_or(&[], D::entities);

        Self {
            fetch,
            entities: entities.iter(),
        }
    }

    /// 取下一个匹配实体，同时返回它的 ID。
    pub(crate) fn next_with_entity(&mut self) -> Option<(EntityId, D::Item<'w>)> {
        let fetch = self.fetch?;

        // 驱动 Storage 里可能存在缺少其它查询组件的实体，跳过它们。
        for &entity in &mut self.entities {
            if let Some(item) = D::get(fetch, entity) {
                return Some((entity, item));
            }
        }

        None
    }
}

impl<'w, D: QueryData> Iterator for QueryIter<'w, D> {
    type Item = D::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_with_entity().map(|(_, item)| item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // 驱动 Storage 的剩余实体数是结果数的上界
        (0, Some(self.entities.len()))
    }
}

impl<'w, D: QueryData> Iterator for QueryIterWithEntities<'w, D> {
    type Item = (EntityId, D::Item<'w>);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_with_entity()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// 一次可变查询。
///
/// 由 `World::query_mut` 创建，构造时独占 `World`，并一次性借出查询需要的
/// 全部 Storage。因此查询存活期间不能 spawn/despawn、插入或删除组件。
///
/// 同一个查询中重复访问同一组件类型会在构造时 panic。
pub(crate) struct QueryMut<'w, D: QueryDataMut> {
    storages: Option<D::Fetch<'w>>,
    entities: Vec<EntityId>,
}

impl<'w, D: QueryDataMut> QueryMut<'w, D> {
    pub(crate) fn new(world: &'w mut World) -> Self {
        let Some(storages) = D::fetch(world.components_mut()) else {
            // 查询涉及的某个组件类型从未被插入过，结果必然为空
            return Self {
                storages: None,
                entities: Vec::new(),
            };
        };

        let entities = D::entities(&storages).to_vec();

        Self {
            storages: Some(storages),
            entities,
        }
    }

    /// 取出实体 `entity` 上的数据，缺少任意一个查询组件时返回 [`None`]
    ///
    /// 返回值借自查询自身，因此不能比 `&mut self` 活得更久
    pub fn get(&mut self, entity: EntityId) -> Option<D::Item<'_>> {
        D::get(self.storages.as_mut()?, entity)
    }

    /// 遍历所有满足查询条件的实体
    ///
    /// 回调中的引用只在本次调用内有效；如果需要在回调外保存结果，
    /// 请保存拷贝而不是引用
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: for<'a> FnMut(D::Item<'a>),
    {
        self.for_each_impl(|_, item| f(item));
    }

    /// 与 [`QueryMut::for_each`] 相同，但额外把实体 ID 传给回调
    pub fn for_each_with_entity<F>(&mut self, f: F)
    where
        F: for<'a> FnMut(EntityId, D::Item<'a>),
    {
        self.for_each_impl(f);
    }

    fn for_each_impl<F>(&mut self, mut f: F)
    where
        F: for<'a> FnMut(EntityId, D::Item<'a>),
    {
        let Some(storages) = self.storages.as_mut() else {
            return;
        };

        for &entity in &self.entities {
            if let Some(item) = D::get(storages, entity) {
                f(entity, item);
            }
        }
    }

    /// 驱动 Storage 中的实体列表，**不保证**每个实体都拥有查询的全部组件
    ///
    /// 列表来自实体数最少的那个 Storage，因此其中可能包含缺少其它组件的实体；
    /// 对这类实体 [`QueryMut::get`] 会返回 [`None`]。结构变更在查询存活期间
    /// 被禁止，所以这个列表是稳定的
    pub fn driver_entities(&self) -> &[EntityId] {
        &self.entities
    }
}

/// 系统执行的阶段
///
/// 调度器运行系统时，将会按顺序执行每个阶段，在每个阶段内自动并行执行系统。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Input,
    Update,
    Physics,
    PostUpdate,
    Graphics,
}

pub const PHASE_COUNT: usize = 5;

// 表达系统本身的结构
pub struct System {
    pub name: &'static str,
    pub phase: Phase,
    pub access: Access,
    pub run: Box<dyn FnMut(&mut World)>,
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

    #[derive(Debug, PartialEq)]
    struct Acceleration {
        x: f32,
        y: f32,
    }

    impl Component for Acceleration {}

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

        // 只有 b 同时拥有 Velocity
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

        // Position 很多、Velocity 很少，无论以哪个 Storage 驱动都必须结果正确
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
        world.insert(a, Acceleration { x: 3.0, y: 3.0 });

        world.insert(b, Position { x: 4.0, y: 4.0 });
        world.insert(b, Velocity { x: 5.0, y: 5.0 });
        world.insert(b, Acceleration { x: 6.0, y: 6.0 });

        let sums: Vec<_> = world
            .query::<(&Position, &Velocity, &Acceleration)>()
            .iter()
            .map(|(position, velocity, acceleration)| position.x + velocity.x + acceleration.x)
            .collect();

        assert_eq!(sums.len(), 2);
        assert!(sums.contains(&6.0));
        assert!(sums.contains(&15.0));
    }

    #[test]
    fn query_iter_allows_duplicate_read_components() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        // 只读查询允许重复类型，得到的是同一个组件的多个共享引用
        let tuple = world.query::<(&Position, &Position)>().iter().next();

        let (first, second) = tuple.expect("entity should match");

        assert_eq!(first, second);
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn query_iter_is_empty_when_a_tuple_storage_is_missing() {
        let mut world = World::new();

        // Position 的 Storage 存在且非空，Velocity 从未插入过
        let entity = world.spawn();
        world.insert(entity, Position { x: 1.0, y: 2.0 });

        assert_eq!(world.query::<(&Position, &Velocity)>().iter().count(), 0);
    }

    #[test]
    fn query_iter_with_entities_pairs_entity_with_item() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();

        world.insert(a, Position { x: 1.0, y: 0.0 });
        world.insert(b, Position { x: 2.0, y: 0.0 });
        world.insert(c, Position { x: 3.0, y: 0.0 });
        world.insert(b, Velocity { x: 0.0, y: 0.0 });

        let mut pairs: Vec<_> = world
            .query::<(&Position, &Velocity)>()
            .iter_with_entities()
            .map(|(entity, (position, _))| (entity, position.x))
            .collect();

        pairs.sort_by_key(|(entity, _)| entity.as_u32());

        assert_eq!(pairs, vec![(b, 2.0)]);
    }

    #[test]
    fn multiple_queries_can_coexist() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });
        world.insert(entity, Velocity { x: 3.0, y: 4.0 });

        // 只读查询只借用 &World，因此可以同时存在
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

        // 取出的引用绑定在 World 的借用上，而不是 Query 自身的借用上
        let position = world.query::<&Position>().get(entity).unwrap();

        assert_eq!(position.x, 1.0);
        assert_eq!(position.y, 2.0);
    }

    // ------------------------------------------------------------
    // QueryMut
    // ------------------------------------------------------------

    #[test]
    fn query_mut_get_modifies_component() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        {
            let mut query = world.query_mut::<&mut Position>();
            let position = query.get(entity).unwrap();

            position.x = 10.0;
            position.y = 20.0;
        }

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 10.0, y: 20.0 })
        );
    }

    #[test]
    fn query_mut_get_missing_component_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();

        let mut query = world.query_mut::<&mut Position>();

        assert!(query.get(entity).is_none());
    }

    #[test]
    fn query_mut_get_on_dead_entity_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });
        world.despawn(entity);

        let mut query = world.query_mut::<&mut Position>();

        assert!(query.get(entity).is_none());
    }

    #[test]
    fn query_mut_for_each_updates_all_entities() {
        let mut world = World::new();

        let entities: Vec<_> = (0..10).map(|_| world.spawn()).collect();

        for &entity in &entities {
            world.insert(entity, Position { x: 1.0, y: 2.0 });
        }

        world
            .query_mut::<&mut Position>()
            .for_each(|position| position.x += 1.0);

        for &entity in &entities {
            assert_eq!(world.get::<Position>(entity).unwrap().x, 2.0);
        }
    }

    #[test]
    fn query_mut_for_each_can_run_multiple_times() {
        let mut world = World::new();

        let entity = world.spawn();
        world.insert(entity, Position { x: 1.0, y: 2.0 });

        let mut query = world.query_mut::<&mut Position>();

        query.for_each(|position| position.x += 1.0);
        query.for_each(|position| position.y += 1.0);

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 2.0, y: 3.0 })
        );
    }

    #[test]
    fn query_mut_for_each_with_entity_reports_entities() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();

        world.insert(a, Position { x: 1.0, y: 0.0 });
        world.insert(b, Position { x: 2.0, y: 0.0 });
        world.insert(c, Position { x: 3.0, y: 0.0 });
        world.insert(b, Velocity { x: 0.0, y: 0.0 });

        let mut seen = Vec::new();

        world
            .query_mut::<(&Position, &mut Velocity)>()
            .for_each_with_entity(|entity, (position, velocity)| {
                velocity.x = position.x;
                seen.push(entity);
            });

        assert_eq!(seen, vec![b]);
        assert_eq!(world.get::<Velocity>(b), Some(&Velocity { x: 2.0, y: 0.0 }));
    }

    #[test]
    fn query_mut_mixed_read_write() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });
        world.insert(entity, Velocity { x: 3.0, y: 4.0 });

        world
            .query_mut::<(&Velocity, &mut Position)>()
            .for_each(|(velocity, position)| {
                position.x += velocity.x;
                position.y += velocity.y;
            });

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 4.0, y: 6.0 })
        );
        assert_eq!(
            world.get::<Velocity>(entity),
            Some(&Velocity { x: 3.0, y: 4.0 })
        );
    }

    #[test]
    fn query_mut_tuple_writes_multiple_components() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 0.0, y: 0.0 });
        world.insert(entity, Velocity { x: 3.0, y: 4.0 });

        world
            .query_mut::<(&mut Position, &mut Velocity)>()
            .for_each(|(position, velocity)| {
                position.x = velocity.x;
                position.y = velocity.y;
                velocity.x = 0.0;
                velocity.y = 0.0;
            });

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 3.0, y: 4.0 })
        );
        assert_eq!(
            world.get::<Velocity>(entity),
            Some(&Velocity { x: 0.0, y: 0.0 })
        );
    }

    #[test]
    fn query_mut_skips_entities_missing_component() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();

        world.insert(a, Position { x: 1.0, y: 1.0 });
        world.insert(b, Position { x: 2.0, y: 2.0 });
        world.insert(b, Velocity { x: 1.0, y: 1.0 });

        world
            .query_mut::<(&Position, &mut Velocity)>()
            .for_each(|(position, velocity)| {
                velocity.x = position.x;
            });

        assert_eq!(world.get::<Position>(a), Some(&Position { x: 1.0, y: 1.0 }));
        assert_eq!(world.get::<Velocity>(b), Some(&Velocity { x: 2.0, y: 1.0 }));
    }

    #[test]
    fn query_mut_is_empty_when_component_never_inserted() {
        let mut world = World::new();

        world.spawn();
        world.spawn();

        let mut count = 0;

        world.query_mut::<&mut Position>().for_each(|_| {
            count += 1;
        });

        assert_eq!(count, 0);
    }

    #[test]
    fn query_mut_driver_entities_lists_driver_entities() {
        let mut world = World::new();

        let entities: Vec<_> = (0..3).map(|_| world.spawn()).collect();

        for &entity in &entities {
            world.insert(entity, Position { x: 1.0, y: 2.0 });
        }

        let query = world.query_mut::<&mut Position>();
        let mut listed = query.driver_entities().to_vec();
        listed.sort_by_key(|entity| entity.as_u32());

        let mut expected = entities.clone();
        expected.sort_by_key(|entity| entity.as_u32());

        assert_eq!(listed, expected);
    }

    #[test]
    fn query_mut_driver_entities_may_include_entities_missing_components() {
        let mut world = World::new();

        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();

        // Position 与 Velocity 各有两个实体，平局按元组顺序选 Position 驱动
        world.insert(a, Position { x: 1.0, y: 0.0 });
        world.insert(b, Position { x: 2.0, y: 0.0 });
        world.insert(b, Velocity { x: 0.0, y: 0.0 });
        world.insert(c, Velocity { x: 0.0, y: 0.0 });

        let mut query = world.query_mut::<(&Position, &mut Velocity)>();
        let listed = query.driver_entities().to_vec();

        // 驱动列表包含 a，但 a 没有 Velocity，get 必须返回 None
        assert!(listed.contains(&a));
        assert!(query.get(a).is_none());
        assert!(query.get(b).is_some());
    }

    #[test]
    #[should_panic(expected = "same component type")]
    fn query_mut_rejects_duplicate_component_types() {
        let mut world = World::new();

        world.spawn();

        let _ = world.query_mut::<(&mut Position, &mut Position)>();
    }

    #[test]
    fn query_mut_read_only_component() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        {
            let mut query = world.query_mut::<&Position>();
            assert_eq!(query.get(entity), Some(&Position { x: 1.0, y: 2.0 }));
        }

        let mut sum = 0.0;

        world.query_mut::<&Position>().for_each(|position| {
            sum += position.x;
        });

        assert_eq!(sum, 1.0);
    }

    #[test]
    fn query_mut_requires_all_tuple_components() {
        let mut world = World::new();

        let position_only = world.spawn();
        let velocity_only = world.spawn();
        let both = world.spawn();

        world.insert(position_only, Position { x: 1.0, y: 2.0 });
        world.insert(velocity_only, Velocity { x: 3.0, y: 4.0 });
        world.insert(both, Position { x: 5.0, y: 6.0 });
        world.insert(both, Velocity { x: 7.0, y: 8.0 });

        let mut query = world.query_mut::<(&Position, &mut Velocity)>();

        assert!(query.get(position_only).is_none());
        assert!(query.get(velocity_only).is_none());

        let (position, velocity) = query.get(both).unwrap();

        assert_eq!(position.x, 5.0);
        assert_eq!(position.y, 6.0);
        assert_eq!(velocity.x, 7.0);
        assert_eq!(velocity.y, 8.0);
    }

    #[test]
    fn query_mut_three_component_tuple() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });
        world.insert(entity, Velocity { x: 10.0, y: 20.0 });
        world.insert(entity, Acceleration { x: 100.0, y: 200.0 });

        world
            .query_mut::<(&mut Position, &Velocity, &mut Acceleration)>()
            .for_each(|(position, velocity, acceleration)| {
                position.x += velocity.x;
                position.y += velocity.y;
                acceleration.x += velocity.y;
            });

        assert_eq!(
            world.get::<Position>(entity),
            Some(&Position { x: 11.0, y: 22.0 })
        );
        assert_eq!(
            world.get::<Acceleration>(entity),
            Some(&Acceleration { x: 120.0, y: 200.0 })
        );
    }

    #[test]
    fn query_mut_uses_smallest_storage_as_driver() {
        let mut world = World::new();
        let entities: Vec<_> = (0..64).map(|_| world.spawn()).collect();

        for (index, &entity) in entities.iter().enumerate() {
            world.insert(
                entity,
                Position {
                    x: index as f32,
                    y: 0.0,
                },
            );
        }

        world.insert(entities[10], Velocity { x: 1.0, y: 2.0 });
        world.insert(entities[40], Velocity { x: 3.0, y: 4.0 });

        let mut count = 0;

        world
            .query_mut::<(&Position, &mut Velocity)>()
            .for_each(|(position, velocity)| {
                velocity.x = position.x;
                count += 1;
            });

        assert_eq!(count, 2);
        assert_eq!(world.get::<Velocity>(entities[10]).unwrap().x, 10.0);
        assert_eq!(world.get::<Velocity>(entities[40]).unwrap().x, 40.0);
    }

    #[test]
    fn query_mut_for_each_skips_despawned_entities() {
        let mut world = World::new();
        let entities: Vec<_> = (0..5).map(|_| world.spawn()).collect();

        for &entity in &entities {
            world.insert(entity, Position { x: 1.0, y: 2.0 });
        }

        world.despawn(entities[2]);

        let mut count = 0;

        world.query_mut::<&mut Position>().for_each(|_| {
            count += 1;
        });

        assert_eq!(count, 4);
    }

    #[test]
    fn query_mut_stale_entity_id_cannot_access_reused_entity() {
        let mut world = World::new();

        let old = world.spawn();
        world.insert(old, Position { x: 1.0, y: 2.0 });
        world.despawn(old);

        let new = world.spawn();
        world.insert(new, Position { x: 10.0, y: 20.0 });

        let mut query = world.query_mut::<&mut Position>();

        assert!(query.get(old).is_none());

        let position = query.get(new).unwrap();

        assert_eq!(position.x, 10.0);
        assert_eq!(position.y, 20.0);
    }

    #[test]
    fn query_mut_missing_one_component_storage_is_empty() {
        let mut world = World::new();
        let entity = world.spawn();

        world.insert(entity, Position { x: 1.0, y: 2.0 });

        let mut count = 0;

        world
            .query_mut::<(&Position, &mut Velocity)>()
            .for_each(|_| {
                count += 1;
            });

        assert_eq!(count, 0);
    }

    #[test]
    #[should_panic(expected = "same component type")]
    fn query_mut_rejects_mixed_duplicate_component_types() {
        let mut world = World::new();

        world.spawn();

        let _ = world.query_mut::<(&Position, &mut Position)>();
    }
}
