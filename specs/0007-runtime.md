# Spec 0007: Runtime

## Status

Final baseline.

## Purpose

This spec defines runtime requirements for scripts, host bindings, scheduling, state updates, and renderer coordination.

## Runtime Responsibilities

The runtime must provide:

- script module loading,
- host binding registration,
- event dispatch,
- state update scheduling,
- timers,
- async task handling,
- render invalidation,
- lifecycle hooks,
- error reporting,
- capability enforcement.

## Script Requirements

Script execution must support:

- JavaScript modules,
- TypeScript-compiled output,
- promises,
- timers,
- host calls,
- structured data exchange,
- deterministic teardown,
- runtime interruption for runaway execution.

## Host Binding Requirements

Host bindings must be capability-scoped and typed.

A host binding must define:

- name,
- input schema,
- output schema,
- required capability,
- sync or async behavior,
- error type,
- lifecycle availability.

## Scheduling Requirements

The scheduler must coordinate:

- script jobs,
- host callbacks,
- UI events,
- render invalidations,
- animation ticks,
- timers,
- shutdown cancellation.

## Plugin Runtime Constraint

The audio thread must never execute script code, rendering code, filesystem calls, network calls, or blocking synchronization.

## Acceptance Criteria

- Runtime host bindings are capability-scoped.
- Script jobs can trigger scene updates.
- Scheduler can request rendering without blocking plugin audio processing.
- Runtime errors are reported without crashing host processes.
