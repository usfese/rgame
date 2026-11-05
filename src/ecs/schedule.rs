use crate::ecs::{access::Access, command::WorldCommands, system::*, world::World};

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

    pub fn add_system<D, F>(
        &mut self,
        name: &'static str,
        phase: Phase,
        extra: Access,
        mut system: F,
    ) where
        D: QueryDataMut,
        F: for<'w> FnMut(QueryMut<'w, D>, &mut WorldCommands) + 'static,
    {
        let mut access = Access::new();
        D::access(&mut access); // 这个函数是把D的访问集加到参数上
        access.merge(&extra);
        self.systems[phase as usize].push(System {
            name,
            phase,
            access,
            run: Box::new(move |world| {
                let mut commands = WorldCommands::new();

                {
                    let query = world.query_mut::<D>();
                    system(query, &mut commands);
                }

                commands.apply(world);
            }),
        });

        self.sorted = false;
    }

    pub fn run(&mut self, world: &mut World) {
        // if !self.sorted {
        //     self.sort();
        // }
        // 先普通顺序执行先，以后再考虑自动并行执行
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::component::Component;
    use std::any::TypeId;

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
    struct Tag(u32);

    impl Component for Tag {}

    #[test]
    fn system_can_spawn_through_commands() {
        let mut world = World::new();
        let mut schedule = Schedule::new();

        schedule.add_system::<&Tag, _>(
            "spawn",
            Phase::Update,
            Access::new().write::<Tag>(),
            |_query, commands| {
                commands.push(|world| {
                    let entity = world.spawn();
                    world.insert(entity, Tag(1));
                });
            },
        );

        schedule.run(&mut world);

        assert_eq!(world.query::<&Tag>().iter().count(), 1);
    }

    #[test]
    fn commands_are_applied_between_systems() {
        let mut world = World::new();
        let mut schedule = Schedule::new();

        schedule.add_system::<&Tag, _>(
            "spawn",
            Phase::Update,
            Access::new().write::<Tag>(),
            |_query, commands| {
                commands.push(|world| {
                    let entity = world.spawn();
                    world.insert(entity, Tag(7));
                });
            },
        );

        schedule.add_system::<&Tag, _>(
            "observe",
            Phase::Update,
            Access::new(),
            |mut query, _commands| {
                let mut seen = 0;

                query.for_each(|tag| {
                    assert_eq!(tag.0, 7);
                    seen += 1;
                });

                assert_eq!(seen, 1);
            },
        );

        schedule.run(&mut world);
    }

    #[test]
    fn commands_are_drained_after_each_run() {
        let mut world = World::new();
        let mut schedule = Schedule::new();

        schedule.add_system::<&Tag, _>(
            "spawn",
            Phase::Update,
            Access::new().write::<Tag>(),
            |_query, commands| {
                commands.push(|world| {
                    let entity = world.spawn();
                    world.insert(entity, Tag(1));
                });
            },
        );

        schedule.run(&mut world);
        assert_eq!(world.query::<&Tag>().iter().count(), 1);

        schedule.run(&mut world);
        assert_eq!(world.query::<&Tag>().iter().count(), 2);
    }

    #[test]
    fn command_can_despawn_entity() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Tag(1));

        let mut schedule = Schedule::new();

        schedule.add_system::<&Tag, _>(
            "despawn",
            Phase::Update,
            Access::new().write::<Tag>(),
            |mut query, commands| {
                let mut target = None;

                query.for_each_with_entity(|entity, _| {
                    target = Some(entity);
                });

                if let Some(entity) = target {
                    commands.push(move |world| {
                        world.despawn(entity);
                    });
                }
            },
        );

        schedule.run(&mut world);

        assert!(!world.is_alive(entity));
        assert_eq!(world.query::<&Tag>().iter().count(), 0);
    }

    #[test]
    fn add_system_merges_query_and_extra_access() {
        let mut schedule = Schedule::new();

        schedule.add_system::<(&Velocity, &mut Position), _>(
            "movement",
            Phase::Update,
            Access::new().read::<Tag>(),
            |_query, _commands| {},
        );

        let system = &schedule.systems[Phase::Update as usize][0];

        assert!(system.access.reads().contains(&TypeId::of::<Velocity>()));
        assert!(system.access.writes().contains(&TypeId::of::<Position>()));
        assert!(system.access.reads().contains(&TypeId::of::<Tag>()));
    }

    #[test]
    fn add_system_stores_system_in_phase() {
        let mut schedule = Schedule::new();

        schedule.add_system::<&Tag, _>("input", Phase::Input, Access::new(), |_, _| {});
        schedule.add_system::<&Tag, _>("update", Phase::Update, Access::new(), |_, _| {});

        assert_eq!(schedule.systems[Phase::Input as usize].len(), 1);
        assert_eq!(schedule.systems[Phase::Update as usize].len(), 1);
        assert_eq!(schedule.systems[Phase::Physics as usize].len(), 0);
    }
}
