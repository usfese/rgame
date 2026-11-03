use std::any::TypeId;

#[derive(Default, Clone, Debug)]
pub struct Access {
    reads: Vec<TypeId>,
    writes: Vec<TypeId>,
}

impl Access {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read<T: 'static>(mut self) -> Self {
        self.reads.push(TypeId::of::<T>());
        self
    }
    pub fn write<T: 'static>(mut self) -> Self {
        self.writes.push(TypeId::of::<T>());
        self
    }
    pub fn merge(&mut self, other: &Access) {
        self.reads.extend_from_slice(&other.reads);
        self.writes.extend_from_slice(&other.writes);
    }

    pub fn reads(&self) -> &Vec<TypeId> {
        &self.reads
    }

    pub fn writes(&self) -> &Vec<TypeId> {
        &self.writes
    }
}
