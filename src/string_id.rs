use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StringId(u32);

struct StringInterner {
    strings: Vec<&'static str>,
    lookup: HashMap<&'static str, StringId>,
}

impl StringInterner {
    fn new() -> Self {
        Self {
            strings: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    fn intern(&mut self, value: &str) -> StringId {
        let id = StringId(self.strings.len() as u32);

        let value: &'static str = Box::leak(value.to_owned().into_boxed_str());

        self.strings.push(value);
        self.lookup.insert(value, id);

        id
    }

    fn get(&self, id: StringId) -> &'static str {
        self.strings[id.0 as usize]
    }
}

static INTERNER: OnceLock<RwLock<StringInterner>> = OnceLock::new();

fn interner() -> &'static RwLock<StringInterner> {
    INTERNER.get_or_init(|| RwLock::new(StringInterner::new()))
}

impl StringId {
    pub fn new(value: &str) -> Self {
        {
            let interner = interner().read().unwrap();

            if let Some(&id) = interner.lookup.get(value) {
                return id;
            }
        }

        let mut interner = interner().write().unwrap();

        if let Some(&id) = interner.lookup.get(value) {
            return id;
        }

        interner.intern(value)
    }

    pub fn as_str(self) -> &'static str {
        let interner = interner().read().unwrap();
        interner.get(self)
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn from_u32(value: u32) -> Self {
        Self(value)
    }
}
