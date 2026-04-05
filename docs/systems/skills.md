# Skills

## Overview

Skills are executable capabilities that can be attached to the orchestrator. They follow the same trait-and-registry pattern as classifiers but are action-oriented rather than analysis-oriented — a classifier observes, a skill acts.

## Design

A skill has a name, a human-readable description, a trigger configuration, and an execute function. The trigger determines when the skill runs: on demand (explicitly invoked), when an agent session starts, or when an agent session ends. Execution receives context about which agent is involved and any arguments, and returns a result indicating success or failure with a flexible JSON payload.

The registry holds all registered skills, supports enable/disable toggling, and provides dispatch methods for both named execution and trigger-based batch execution.

## Current Status

Framework only. No built-in skills are registered. The system is designed to be extended by implementing the skill trait and registering instances during app initialization, then wiring the session start/end trigger points in the event loop.
