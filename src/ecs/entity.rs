const SLOT_BITS: u32 = 20;
const GENERATION_BITS: u32 = 12;

const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;
const GENERATION_MASK: u32 = (1 << GENERATION_BITS) - 1;

const INVALID_GENERATION: u16 = GENERATION_MASK as u16;

use std::collections::VecDeque;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityId(u32);

impl EntityId {
    #[inline]
    pub const fn new(slot: u32, generation: u32) -> Self {
        debug_assert!(slot <= SLOT_MASK);
        debug_assert!(generation <= GENERATION_MASK);

        Self(slot | (generation << SLOT_BITS))
    }

    #[inline]
    pub const fn slot(self) -> u32 {
        self.0 & SLOT_MASK
    }

    #[inline]
    pub const fn generation(self) -> u32 {
        (self.0 >> SLOT_BITS) & GENERATION_MASK
    }

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

pub struct EntityManager {
    generations: Vec<u16>,
    free: VecDeque<u32>,
    entity_count: usize,
}

impl EntityManager {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            free: VecDeque::new(),
            entity_count: 0,
        }
    }

    pub fn spawn(&mut self) -> EntityId {
        self.entity_count += 1;

        while let Some(slot) = self.free.pop_front() {
            let generation = self.generations[slot as usize];

            if generation == INVALID_GENERATION {
                continue;
            }

            return EntityId::new(slot, generation as u32);
        }

        let slot = self.generations.len();

        assert!(
            slot <= SLOT_MASK as usize,
            "maximum number of entity slots exceeded"
        );

        self.generations.push(0);

        EntityId::new(slot as u32, 0)
    }

    pub fn destroy(&mut self, entity: EntityId) -> bool {
        let slot = entity.slot() as usize;

        let Some(&generation) = self.generations.get(slot) else {
            return false;
        };

        if generation as u32 != entity.generation() {
            return false;
        }

        if generation == INVALID_GENERATION - 1 {
            self.generations[slot] = INVALID_GENERATION;
            return true;
        }

        self.generations[slot] = generation + 1;
        self.free.push_back(slot as u32);
        self.entity_count -= 1;

        true
    }

    #[inline]
    pub fn is_alive(&self, entity: EntityId) -> bool {
        self.generations
            .get(entity.slot() as usize)
            .is_some_and(|&generation| generation as u32 == entity.generation())
    }

    #[inline]
    pub fn entity_count(&self) -> usize {
        self.entity_count
    }
}

impl Default for EntityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_pack_and_unpack() {
        let entity = EntityId::new(123, 456);

        assert_eq!(entity.slot(), 123);
        assert_eq!(entity.generation(), 456);
    }

    #[test]
    fn entity_id_max_values() {
        let entity = EntityId::new(SLOT_MASK, GENERATION_MASK);

        assert_eq!(entity.slot(), SLOT_MASK);
        assert_eq!(entity.generation(), GENERATION_MASK);
    }

    #[test]
    fn spawn_creates_alive_entity() {
        let mut manager = EntityManager::new();

        let entity = manager.spawn();

        assert!(manager.is_alive(entity));
        assert_eq!(entity.slot(), 0);
        assert_eq!(entity.generation(), 0);
    }

    #[test]
    fn spawn_creates_unique_entities() {
        let mut manager = EntityManager::new();

        let a = manager.spawn();
        let b = manager.spawn();

        assert_ne!(a, b);
        assert!(manager.is_alive(a));
        assert!(manager.is_alive(b));
    }

    #[test]
    fn destroy_kills_entity() {
        let mut manager = EntityManager::new();

        let entity = manager.spawn();

        assert!(manager.is_alive(entity));

        assert!(manager.destroy(entity));

        assert!(!manager.is_alive(entity));
    }

    #[test]
    fn destroy_invalid_entity_returns_false() {
        let mut manager = EntityManager::new();

        let entity = EntityId::new(123, 0);

        assert!(!manager.destroy(entity));
        assert!(!manager.is_alive(entity));
    }

    #[test]
    fn destroy_twice_returns_false() {
        let mut manager = EntityManager::new();

        let entity = manager.spawn();

        assert!(manager.destroy(entity));
        assert!(!manager.destroy(entity));
    }

    #[test]
    fn destroyed_slot_is_reused() {
        let mut manager = EntityManager::new();

        let first = manager.spawn();

        assert!(manager.destroy(first));

        let second = manager.spawn();

        assert_eq!(first.slot(), second.slot());
        assert_ne!(first, second);
        assert_eq!(second.generation(), first.generation() + 1);

        assert!(!manager.is_alive(first));
        assert!(manager.is_alive(second));
    }

    #[test]
    fn old_entity_id_does_not_become_alive_again() {
        let mut manager = EntityManager::new();

        let old = manager.spawn();
        manager.destroy(old);

        let new = manager.spawn();

        assert_ne!(old, new);

        assert!(!manager.is_alive(old));
        assert!(manager.is_alive(new));
    }

    #[test]
    fn destroying_one_entity_does_not_affect_others() {
        let mut manager = EntityManager::new();

        let a = manager.spawn();
        let b = manager.spawn();
        let c = manager.spawn();

        manager.destroy(b);

        assert!(manager.is_alive(a));
        assert!(!manager.is_alive(b));
        assert!(manager.is_alive(c));
    }

    #[test]
    fn generation_increases_each_reuse() {
        let mut manager = EntityManager::new();

        let first = manager.spawn();
        assert_eq!(first.generation(), 0);

        manager.destroy(first);

        let second = manager.spawn();
        assert_eq!(second.generation(), 1);

        manager.destroy(second);

        let third = manager.spawn();
        assert_eq!(third.generation(), 2);

        assert!(!manager.is_alive(first));
        assert!(!manager.is_alive(second));
        assert!(manager.is_alive(third));
    }

    #[test]
    fn entity_id_is_copy() {
        let entity = EntityId::new(10, 20);

        let copy = entity;

        assert_eq!(entity, copy);
    }
}
