# Classifiers

## Overview

Classifiers analyze agent traces to detect patterns — looping behavior, token budget overruns, error cascades, or any other signal extractable from conversation data. They are trait-based plugins: implement the trait, register with the registry, and the system dispatches automatically.

## Design

A classifier has a name, a trigger configuration, and an analysis function. The trigger determines when it runs: on each new turn (incremental), when a trace completes (on-complete), or both. The analysis function receives a trace and its turns, and returns zero or more detection results with a flexible JSON payload.

The registry holds all registered classifiers and dispatches to them based on trigger type. Only enabled classifiers run. Disabled state persists across restarts via the config file.

## Trigger Points

Classification runs at two points:

1. **Incremental** — each time a new turn arrives, the registry runs all incremental classifiers against the current trace state. This enables real-time detection while the agent is still running.

2. **On-complete** — when a trace finishes (either normally or by abort), the registry runs all on-complete classifiers. This enables analysis that requires the full conversation.

Results from both trigger points are persisted to the database and appear in the Classifiers page.

## ML Integration

The ML crate provides a decision tree classifier as a concrete implementation. It extracts 14 numerical features from a trace (token counts, turn counts, tool usage patterns, error rates, timing) and predicts a label using a trained linfa decision tree model.

The training pipeline takes labeled trace data, extracts features, trains a model, and produces a serializable model that can be persisted and reloaded. Models are versioned — each retraining creates a new version.

## Current Status

The framework is complete but no classifiers are registered in the app yet. The ML training pipeline exists but is not wired to any UI or automated workflow. The system is designed to be extended by implementing the classifier trait and registering instances during app initialization.
