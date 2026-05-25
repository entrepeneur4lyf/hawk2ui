# Hawk2UI Performance Policy

Performance gates are release gates, not advisory notes.

## Budgets

Budgets live in `performance/budgets.toml`. Every release-gating benchmark must map to a named budget with a unit, target, maximum, fixture, and release gate flag.

## Benchmarks

Every benchmark case must report an observed value in the same unit as its
budget. A case without a measurement fails the gate, and a measurement above the
configured `maximum` fails release readiness.

Run the benchmark gates with:

- `rtk cargo bench -p hawk2ui-perf --bench startup`
- `rtk cargo bench -p hawk2ui-perf --bench layout`
- `rtk cargo bench -p hawk2ui-perf --bench render`
- `rtk cargo bench -p hawk2ui-perf --bench render_baseline -- --quick`
- `rtk cargo bench -p hawk2ui-perf --bench runtime`
- `rtk cargo bench -p hawk2ui-perf --bench plugin_realtime`

## Realtime Safety

Audio-thread contexts must deny allocation, blocking waits, filesystem access, network access, script execution, and rendering work. Only preallocated non-blocking writes are allowed in the initial realtime guard.
