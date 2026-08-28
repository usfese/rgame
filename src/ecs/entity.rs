const SLOT_BITS: u32 = 20;
const GENERATION_BITS: u32 = 12;

const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;
const GENERATION_MASK: u32 = (1 << GENERATION_BITS) - 1;

const INVALID_GENERATION: u16 = GENERATION_MASK as u16;

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
    free: Vec<u32>,
}

impl EntityManager {
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn spawn(&mut self) -> EntityId {
        while let Some(slot) = self.free.pop() {
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
        self.free.push(slot as u32);

        true
    }

    #[inline]
    pub fn is_alive(&self, entity: EntityId) -> bool {
        self.generations
            .get(entity.slot() as usize)
            .is_some_and(|&generation| generation as u32 == entity.generation())
    }
}

impl Default for EntityManager {
    fn default() -> Self {
        Self::new()
    }
}
