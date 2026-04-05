# UI Architecture

## Pages

The application has four pages, navigated via a tab bar at the top of the screen:

| Page | Content |
|---|---|
| **Home** | Agent terminals, file tree, token chart, status bar |
| **Classifiers** | Registered classifiers with toggle, recent classification results |
| **Database** | SQLite table browser with paginated query results |
| **Settings** | Configuration editor |

Switching pages changes which widgets render in the content area below the tab bar.

## Grid Layout

The home page uses a 12-column, 5-row grid system. The left sidebar (2 columns) holds monitoring widgets stacked vertically — agent list, token chart, recent activity, file tree, and keyboard shortcuts. The right side (10 columns) is the agent pane, spanning all rows, showing live terminal output.

The grid layout is configurable via a JSON file and computed by the grid crate into positioned rectangles used for both rendering and mouse hit-testing.

## Widget System

All UI components implement a common trait with three responsibilities: rendering into an assigned screen area, handling scroll events, and handling click events. Click handling returns an action value when the interaction requires a state change (selecting an agent, toggling a classifier, saving settings, switching pages).

Widgets receive a read-only context containing all application state — agents, turns, traces, focus, and file tree. They never mutate application state directly. State changes flow back through action return values, which the app dispatches.

## Widgets

| Widget | Page | Purpose |
|---|---|---|
| NavBar | All | Tab bar for page switching |
| AgentList | Home | List of agents with status indicators |
| AgentPane | Home | Terminal output in grid or focused mode |
| TokenChart | Home | Horizontal bar chart of token usage per agent |
| RecentActivity | Home | Most recently modified files with diff stats |
| FileTree | Home | Expandable file/directory tree with activity markers |
| StatusBar | Home | Keyboard shortcut reference |
| SettingsPage | Settings | Config editor with save |
| DatabasePage | Database | Table browser with pagination |
| ClassifiersPage | Classifiers | Classifier status and recent results |

## Input

Global keyboard shortcuts (new agent, close agent, quit, page navigation) are handled before any widget sees the event. Unhandled keys are forwarded to the focused agent's terminal.

Mouse clicks are hit-tested against the tab bar first, then against grid cell rectangles. The matching widget processes the click. Scroll events go directly to the widget under the cursor.

## Focus

The focus system tracks the current page, which panel has focus (agent list or agent pane), and which agent is active. Focus affects visual styling (active agent gets highlighted borders) and input routing (keyboard input goes to the active agent's terminal).
