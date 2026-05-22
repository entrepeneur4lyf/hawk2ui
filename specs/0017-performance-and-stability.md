# Spec 0017: Performance And Stability

## Status

Final baseline.

## Purpose

This spec defines performance, resource usage, runtime stability, and realtime safety requirements for production Hawk2UI releases.

## Budget Requirements

Performance budgets must exist for:

- cold startup,
- artifact loading,
- first frame generation,
- layout calculation,
- text measurement,
- scene export,
- renderer command generation,
- frame presentation,
- event dispatch,
- runtime scheduling,
- memory use,
- package size.

Each budget must define measurement unit, fixture, target value, maximum value, and release gate status.

## Rendering Stability Requirements

Rendering must remain stable under:

- repeated resize,
- DPI changes,
- window minimize and restore,
- host-driven plugin resize,
- dense scene updates,
- animated properties,
- text changes,
- asset cache invalidation,
- custom draw surface invalidation.

## Runtime Stability Requirements

Runtime behavior must remain stable under:

- repeated event dispatch,
- batched state updates,
- timers,
- async task completion,
- shutdown cancellation,
- lifecycle teardown,
- denied capability calls,
- script interruption.

## Realtime Safety Requirements

Plugin audio processing paths must not perform:

- allocation,
- blocking waits,
- script execution,
- rendering,
- filesystem access,
- network access,
- process access,
- host calls not approved for audio processing.

Realtime visual data transport must tolerate dropped UI frames without blocking audio processing.

## Acceptance Criteria

- Performance budgets are executable release gates.
- Regressions are detected by automated tests or benchmarks.
- Realtime safety violations fail tests.
- Stability fixtures cover resize, DPI, lifecycle, runtime, and rendering stress paths.
