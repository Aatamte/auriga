# Storage

## Overview

Persistence is handled by SQLite through a background thread. The main thread sends write commands via a channel and never blocks on I/O. A separate read-only connection serves UI queries on the main thread.

## Database

Located at `.auriga/auriga.db`, created on first run. The schema is versioned with incremental migrations — each version adds new tables without modifying existing ones. Migration runs automatically on every database open.

## Data Model

The database stores five categories of data:

- **Traces** — one row per agent conversation session, with status, timing, and aggregate token counts
- **Turns** — one row per message in a conversation, with structured content, metadata, and context stored as JSON
- **Classifications** — detection results from classifiers, with flexible JSON payloads
- **Models** — serialized ML models with version tracking and accuracy metrics
- **Training labels** — ground-truth labels mapping traces to classification categories for supervised learning

## Write Path

All writes go through a dedicated background thread. The main thread sends fire-and-forget commands through a channel — it never waits for write confirmation. This keeps the main loop responsive regardless of database I/O latency.

The storage thread processes commands sequentially. Write errors are logged but do not crash the application.

On shutdown, the main thread sends a shutdown command and joins the storage thread to ensure all pending writes complete.

## Read Path

A separate database connection on the main thread handles read queries for the UI. This includes listing traces, browsing tables, loading classification results, and fetching database metadata (table counts, file size). Reads are synchronous but fast — they operate on indexed SQLite tables.

## Error Handling

Storage is designed to be resilient. Write failures are logged and skipped. Missing or corrupt database files trigger a fresh creation with default schema. The application functions without persistence — it degrades gracefully rather than crashing.
