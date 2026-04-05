# File Activity

## Overview

The orchestrator tracks which files each agent modifies, visualizes the file hierarchy with activity markers, and shows recent changes with line-level diff information. This gives operators a real-time view of what agents are doing to the codebase.

## File Tree

An in-memory tree structure represents the project's file hierarchy. Each entry tracks its path, whether it's a file or directory, its depth in the tree, expand/collapse state, lines added and removed, when it was last modified, and which agent last touched it.

The tree maintains cached derived state — visible entries (filtered by expand/collapse) and recent activity (sorted by modification time). Caches are invalidated by a dirty flag on any mutation and recomputed lazily.

## File Watching

A background thread monitors the project directory for filesystem changes using the notify crate. It respects gitignore rules to skip irrelevant directories like `.git/`, `node_modules/`, and `target/`. Change events are sent to the main thread, which updates the file tree.

## Diff Tracking

A separate background thread calculates line-level diffs. When a file change is detected, the main thread sends the file path to the diff thread, which runs git diff and returns the counts of lines added and removed. These are stored on the file entry for display.

## Activity Aging

Files are color-coded by how recently they were modified, using a five-tier scale from red (seconds ago) through yellow, green, and cyan, down to gray (more than ten minutes). This gives an immediate visual sense of where active work is happening.

## Display

Two widgets surface file activity on the home page:

**RecentActivity** shows the most recently modified files with diff stats, agent attribution, and age-based color coding.

**FileTree** shows the full expandable directory tree with the same activity markers. Directories can be expanded and collapsed by clicking.
