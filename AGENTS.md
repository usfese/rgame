# rgame

Single Rust 2024 binary crate: hand-rolled ECS + Vulkan renderer (`ash`) + `winit` windowing. No README, CI, workspace, or external `tests/` directory.

## Commands

- `cargo check --all-targets` — fastest full compile check (also compiles test code)
- `cargo test` — all unit tests
- `cargo test <substring>` — one test, e.g. `cargo test ecs::world::tests::spawn_creates_entity`
- `cargo fmt` / `cargo clippy --all-targets`
- `cargo doc --no-deps` — public API docs
- `cargo run` — **does not start the game**. `Game::game_loop` is an unfinished busy loop whose `handle_input`/`update_all`/`render` are `todo!()`, so it panics immediately. The real `winit` flow lives in `impl ApplicationHandler for Game`, but `main` never creates an event loop.

## Layout

- `src/main.rs` — module declarations; constructs `Game` and calls `game_loop`.
- `src/game.rs` — `Game` and its `ApplicationHandler`; `Renderer::render` and `Renderer::resize` are also `todo!()`.
- `src/renderer.rs` — all Vulkan setup in `Renderer::new`; `MAX_FRAME_LATENCY = 3`. Enables `VK_LAYER_KHRONOS_validation` unconditionally with no fallback, so the machine needs the Vulkan loader and validation layer installed.
- `src/ecs/{entity,component,world,system}.rs` — ECS split by concern.
- `src/string_id.rs` — process-global string interner (`OnceLock<RwLock>`); IDs are only valid within one process run.

## ECS invariants

- `EntityId` packs a 20-bit slot and 12-bit generation (`src/ecs/entity.rs`). `EntityId::new` is `pub(crate)`; normal code must obtain IDs from `World::spawn`, since a free slot's stored generation is the one the next spawn will hand out. Never index component storage by slot alone; `Storage::dense_index_of` also compares the full `EntityId`, which is what prevents stale IDs from seeing a reused slot.
- `Component` is not blanket-implemented. Every component type needs `impl Component for Foo {}`.
- `Storage<T>` is a sparse set. `remove` swaps the last element into the hole, so it must update the moved entity's sparse index (covered by `swap_remove_updates_sparse_index`).
- `World::insert` silently does nothing for dead entities; `despawn` removes every component of that entity.
- Read queries (`World::query`) hold `&World` and can coexist. Mutable queries (`World::query_mut`) hold `&mut World`, so no `spawn`/`despawn`/`insert`/`remove` while one is alive.
- `QueryDataMut` supports `&mut T`, `&T`, and tuples such as `(&Velocity, &mut Position)`. `QueryMut` exposes `get`, `for_each`, `for_each_with_entity`, and `driver_entities` (the driver storage's entity list, which may include entities missing other components); it intentionally has no `std::iter::Iterator` impl because mutable items borrow the query itself (lending pattern). Duplicate component types in one mutable query panic. Read-only `Query` allows duplicate component types and exposes `iter` and `iter_with_entities`.
- `QueryData`, `Query`, `QueryIter`, `QueryIterWithEntities`, `QueryDataMut`, `QueryMut`, `World::query`, and `World::query_mut` are `pub(crate)` because their `Fetch` types expose the private `Storage<T>` type. Making them public requires making `Storage` public or adding an opaque fetch wrapper.
- Tuple query impls are macro-generated for arities 1–12 in `src/ecs/system.rs`: `impl_query_data!` for read queries and `impl_query_data_mut!` for mutable ones.

## Conventions

- Code comments and doc comments are written in Chinese (test function names are English); match that style.
- Tests are inline `#[cfg(test)] mod tests` next to the code. There are no fixtures or integration-test prerequisites.
- Extra lint warnings are configured only at the top of `src/renderer.rs`, not crate-wide.
