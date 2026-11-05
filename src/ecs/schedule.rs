use crate::ecs::{access::Access, system::*, world::World};

pub struct Schedule {
    systems: [Vec<System>; PHASE_COUNT],
    sorted: bool,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: std::array::from_fn(|_| Vec::new()),
            sorted: true,
        }
    }

    pub fn add_system<D, F>(&mut self, name: &'static str, phase: Phase, mut system: F)
    where
        D: QueryDataMut,
        F: for<'w> FnMut(QueryMut<'w, D>) + 'static, // 看来的写法，我再研究研究生命周期
    {
        let mut access = Access::new();
        D::access(&mut access); // 这个函数是把D的访问集加到参数上
        self.systems[phase as usize].push(System {
            name,
            phase,
            access,
            run: Box::new(move |world| {
                let query = world.query_mut::<D>();
                system(query);
            }),
        });

        self.sorted = false;
    }

    pub fn run(&mut self, world: &mut World) {
        // if !self.sorted {
        //     self.sort();
        // }
        // 我们先普通顺序执行先，以后再考虑自动并行执行
        // 一日不把这个整个&mut World拆了，就一日别想并行
        for i in 0..PHASE_COUNT {
            for system in &mut self.systems[i] {
                (system.run)(world);
            }
        }
    }

    // 调度器真的需要排序吗
    pub fn sort(&mut self) {
        todo!();
    }
}
