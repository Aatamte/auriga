# Traces and Turns

## Overview

A **Trace** represents a single agent conversation session. A **Turn** represents one message within that conversation. Together they form the core data model for monitoring and analyzing agent behavior.

## Trace Lifecycle

A trace is created when the first turn arrives for a new session via the Claude log watcher. It accumulates turn counts and token usage as new turns arrive. When the agent session ends, the trace is marked as complete. If the agent is killed or the app shuts down, the trace is marked as aborted.

Traces are the primary unit that classifiers analyze. Both incremental classification (per-turn) and on-complete classification (when the trace finishes) operate on trace data.

## Turns

Each turn captures a single message in the conversation — user input, assistant response, or system message. Turns carry structured content that can be plain text or a sequence of typed blocks: text, thinking, tool use, tool results, and images.

Assistant turns include metadata about the model response: which model was used, why it stopped, and how many tokens it consumed. This metadata feeds token usage tracking and the classifier feature extraction pipeline.

Turns are deduplicated by their UUID from the Claude log source, so the same log entry processed twice does not create duplicate turns.

## Session Mapping

The app needs to associate incoming Claude log entries with the correct agent. It maintains two lookup paths: session ID mapping (for subsequent entries in a known session) and process ID mapping (for discovering which agent owns a new session). Once a session is mapped to an agent, all subsequent turns in that session are automatically associated.

## Persistence

Traces and turns are persisted to SQLite on every turn arrival (for crash safety) and again when the trace completes. On shutdown, all active traces are aborted and flushed to the database. The database also serves as the data source for the Database page in the UI.
