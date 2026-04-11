# MCP Server

## Overview

The auriga runs a Model Context Protocol server so that agents can discover each other and exchange messages. This enables multi-agent coordination without agents needing to know about each other in advance.

## How It Works

An HTTP server listens on a configurable port (default 7850) and accepts JSON-RPC 2.0 requests. When Auriga starts, it writes a `.mcp.json` file to the project root so that Claude Code agents automatically discover and connect to the server. The file is cleaned up on shutdown.

## Capabilities

The server exposes two tools:

**list_agents** — returns all running agents with their ID, name, and current status (idle or working). Agents use this to discover who else is available for coordination.

**send_message** — delivers a message from one agent to another. The message is written directly to the receiving agent's terminal input. This enables agents to share findings, delegate work, or coordinate on tasks.

## Communication Flow

When an agent makes an MCP request, the server thread translates it into an event and sends it to the main thread via a channel with a one-shot response channel attached. The main thread processes the request against live application state (it has access to the full agent store) and sends a response back. The server thread then serializes the response as JSON-RPC and returns it as the HTTP response.

This design ensures the server thread never accesses application state directly — all state access goes through the main thread, maintaining the single-owner concurrency model.
