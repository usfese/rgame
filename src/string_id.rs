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

    fn get(&self, id: StringId) -> Option<&'static str> {
        self.strings.get(id.0 as usize).copied()
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

    /// 通过 [`StringId::new`] 取得该 id 对应的字符串。
    ///
    /// # Panics
    /// `self` 不是由本进程 interner 分配的合法 id 时 panic。
    pub fn as_str(self) -> &'static str {
        let interner = interner().read().unwrap();
        interner
            .get(self)
            .expect("StringId was not created by this process' interner")
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// 从原始 `u32` 恢复 [`StringId`]。
    ///
    /// 只有本进程 interner 已经分配过的编号才有效，其它值（例如跨进程
    /// 保存下来的编号）返回 [`None`]。
    pub fn from_u32(value: u32) -> Option<Self> {
        let interner = interner().read().unwrap();

        if (value as usize) < interner.strings.len() {
            Some(Self(value))
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::thread;

    #[test]
    fn same_string_has_same_id() {
        let a = StringId::new("hello");
        let b = StringId::new("hello");

        assert_eq!(a, b);
    }

    #[test]
    fn different_strings_have_different_ids() {
        let a = StringId::new("hello");
        let b = StringId::new("world");

        assert_ne!(a, b);
    }

    #[test]
    fn as_str_returns_original_string() {
        let id = StringId::new("hello");

        assert_eq!(id.as_str(), "hello");
    }

    #[test]
    fn string_can_be_interned_multiple_times() {
        let a = StringId::new("hello");
        let b = StringId::new("hello");
        let c = StringId::new("hello");

        assert_eq!(a, b);
        assert_eq!(b, c);

        assert_eq!(a.as_str(), "hello");
    }

    #[test]
    fn u32_conversion_round_trip() {
        let id = StringId::new("hello");

        let value = id.as_u32();
        let restored = StringId::from_u32(value).unwrap();

        assert_eq!(id, restored);
        assert_eq!(restored.as_str(), "hello");
    }

    #[test]
    fn from_u32_rejects_unknown_id() {
        // interner 不可能分配到 u32::MAX 号。
        assert_eq!(StringId::from_u32(u32::MAX), None);
    }

    #[test]
    fn string_id_can_be_used_as_hash_map_key() {
        let player = StringId::new("player");
        let enemy = StringId::new("enemy");

        let mut map = HashMap::new();

        map.insert(player, 100);
        map.insert(enemy, 200);

        assert_eq!(map.get(&player), Some(&100));
        assert_eq!(map.get(&enemy), Some(&200));
    }

    #[test]
    fn empty_string_works() {
        let id = StringId::new("");

        assert_eq!(id.as_str(), "");
    }

    #[test]
    fn unicode_string_works() {
        let id = StringId::new("你好，世界");

        assert_eq!(id.as_str(), "你好，世界");
    }

    #[test]
    fn long_string_works() {
        let value = "a".repeat(10_000);

        let id = StringId::new(&value);

        assert_eq!(id.as_str(), value);
    }

    #[test]
    fn different_strings_get_stable_ids() {
        let values = ["apple", "banana", "orange", "player", "enemy", "component"];

        let ids: Vec<_> = values.iter().map(|value| StringId::new(value)).collect();

        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(id.as_str(), values[i]);
        }

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j]);
            }
        }
    }

    #[test]
    fn concurrent_interning_returns_same_id() {
        let value = "concurrent_string";

        let mut handles: Vec<_> = (0..16)
            .map(|_| thread::spawn(move || StringId::new(value)))
            .collect();

        let first = handles.pop().unwrap().join().unwrap();

        for handle in handles {
            assert_eq!(handle.join().unwrap(), first);
        }

        assert_eq!(first.as_str(), value);
    }
}
