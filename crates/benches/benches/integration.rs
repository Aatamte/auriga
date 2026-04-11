use criterion::{black_box, criterion_group, criterion_main, Criterion};
use auriga_core::{AgentId, AgentStore, FileEntry, FileTree, FocusState};
use auriga_grid::Grid;
// grid_to_lines was replaced by render_term — some benches need updating
use auriga_widgets::{RenderContext, Widget, WidgetRegistry};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::path::PathBuf;

use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;

#[derive(Clone)]
struct EventProxy;
impl EventListener for EventProxy {
    fn send_event(&self, _event: alacritty_terminal::event::Event) {}
}

struct TermSize {
    cols: usize,
    lines: usize,
}
impl Dimensions for TermSize {
    fn columns(&self) -> usize {
        self.cols
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn total_lines(&self) -> usize {
        self.lines
    }
}

fn make_agents(n: usize) -> (AgentStore, Vec<AgentId>) {
    let mut store = AgentStore::new();
    let mut ids = Vec::new();
    for _ in 0..n {
        ids.push(store.create("claude"));
    }
    (store, ids)
}

fn make_file_tree(n: usize) -> FileTree {
    let mut tree = FileTree::new(PathBuf::from("/project"));
    let mut entries = Vec::new();
    for i in 0..n / 10 {
        entries.push(FileEntry::dir(
            PathBuf::from(format!("/project/dir{}", i)),
            0,
        ));
        for j in 0..9 {
            entries.push(FileEntry::file(
                PathBuf::from(format!("/project/dir{}/file{}.rs", i, j)),
                1,
            ));
        }
    }
    tree.set_entries(entries);
    tree
}

fn make_term_with_output(rows: u16, cols: u16) -> Term<EventProxy> {
    let config = TermConfig {
        scrolling_history: 1000,
        ..Default::default()
    };
    let size = TermSize {
        cols: cols as usize,
        lines: rows as usize,
    };
    let mut term = Term::new(config, &size, EventProxy);
    let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::new();
    for i in 0..rows {
        let line = format!(
            "\x1b[32m  line {} \x1b[0m some content here with \x1b[1;34mbold blue\x1b[0m text\r\n",
            i
        );
        parser.advance(&mut term, line.as_bytes());
    }
    term
}

fn make_app_state(
    agent_count: usize,
    file_count: usize,
) -> (AgentStore, FocusState, FileTree, Vec<Term<EventProxy>>) {
    let (agents, agent_ids) = make_agents(agent_count);
    let mut focus = FocusState::new();
    if let Some(&first) = agent_ids.first() {
        focus.set_active_agent(first);
    }
    let mut file_tree = make_file_tree(file_count);
    file_tree.refresh_caches();
    let terms: Vec<Term<EventProxy>> = (0..agent_count)
        .map(|_| make_term_with_output(40, 120))
        .collect();
    (agents, focus, file_tree, terms)
}

// -- Event Dispatch Benchmarks --

fn bench_key_event_dispatch(c: &mut Criterion) {
    let (agents, agent_ids) = make_agents(5);
    let mut focus = FocusState::new();
    focus.set_active_agent(agent_ids[0]);

    c.bench_function("key_event_agent_select_cycle", |b| {
        b.iter(|| {
            let ids = agents.ids();
            for id in &ids {
                focus.set_active_agent(*id);
            }
            black_box(&focus);
        });
    });
}

fn bench_mouse_hit_test(c: &mut Criterion) {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 200, 60);
    let cell_rects = grid.compute_rects(area);

    c.bench_function("mouse_hit_test_200x60", |b| {
        b.iter(|| {
            for row in (0..60).step_by(5) {
                for col in (0..200).step_by(10) {
                    for cell in &cell_rects {
                        let r = &cell.rect;
                        if col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height {
                            black_box(&cell.widget);
                            break;
                        }
                    }
                }
            }
        });
    });
}

fn bench_widget_click_dispatch(c: &mut Criterion) {
    let (agents, agent_ids) = make_agents(20);
    let mut focus = FocusState::new();
    focus.set_active_agent(agent_ids[0]);
    let mut file_tree = make_file_tree(500);
    file_tree.refresh_caches();
    let terms: Vec<Term<EventProxy>> = (0..20).map(|_| make_term_with_output(40, 120)).collect();
    let mut widgets = WidgetRegistry::new();

    c.bench_function("widget_click_agent_list_20_agents", |b| {
        b.iter(|| {
            let render_term_fn = |_id: AgentId, _buf: &mut ratatui::buffer::Buffer, _area: Rect| {};
            let turns = auriga_core::TurnStore::new();
            let traces = auriga_core::TraceStore::new();
            let ctx = RenderContext {
                agents: &agents,
                turns: &turns,
                traces: &traces,
                focus: &focus,
                file_tree: &file_tree,
                render_term: &render_term_fn,
                hidden_pages: &[],
            };
            for row in 0..20u16 {
                black_box(widgets.agent_list.handle_click(row, 0, &ctx));
            }
        });
    });
}

// -- Memory / Allocation Benchmarks --

fn bench_agent_store_create_remove_churn(c: &mut Criterion) {
    c.bench_function("agent_store_create_remove_100", |b| {
        b.iter(|| {
            let mut store = AgentStore::new();
            let mut ids = Vec::new();
            for _ in 0..100 {
                ids.push(store.create("claude"));
            }
            for id in ids {
                store.remove(id);
            }
            black_box(&store);
        });
    });
}

fn bench_file_tree_record_activity_churn(c: &mut Criterion) {
    let mut tree = make_file_tree(1000);

    c.bench_function("file_tree_record_activity_1000_files", |b| {
        b.iter(|| {
            for i in 0..100 {
                let dir = i / 9;
                let file = i % 9;
                tree.record_activity(
                    &PathBuf::from(format!("/project/dir{}/file{}.rs", dir, file)),
                    Some(AgentId::from_u128(1)),
                );
            }
            black_box(&tree);
        });
    });
}

fn bench_file_tree_insert_new_files(c: &mut Criterion) {
    c.bench_function("file_tree_insert_50_new_files", |b| {
        b.iter(|| {
            let mut tree = make_file_tree(100);
            for i in 0..50 {
                tree.record_activity(
                    &PathBuf::from(format!("/project/dir0/new_file_{}.rs", i)),
                    Some(AgentId::from_u128(1)),
                );
            }
            black_box(&tree);
        });
    });
}

// -- PTY Throughput Benchmarks --

fn bench_pty_output_processing(c: &mut Criterion) {
    c.bench_function("alacritty_process_1mb_output", |b| {
        let config = TermConfig { scrolling_history: 1000, ..Default::default() };
        let size = TermSize { cols: 120, lines: 40 };
        let mut term = Term::new(config, &size, EventProxy);
        let mut parser = vte::ansi::Processor::<vte::ansi::StdSyncHandler>::new();

        let chunk: Vec<u8> = (0..1024)
            .flat_map(|i| {
                format!(
                    "\x1b[32mline {} \x1b[0m content \x1b[1;31mred\x1b[0m more text here padding\r\n",
                    i
                )
                .into_bytes()
            })
            .collect();

        b.iter(|| {
            parser.advance(&mut term, black_box(&chunk));
            black_box(term.grid());
        });
    });
}

// bench_pty_output_then_render removed — grid_to_lines was replaced by render_term

// -- File Watcher Event Processing --

fn bench_file_event_burst(c: &mut Criterion) {
    c.bench_function("file_event_burst_200_modifications", |b| {
        b.iter(|| {
            let mut tree = make_file_tree(1000);
            for i in 0..200 {
                let dir = i / 9;
                let file = i % 9;
                let path = PathBuf::from(format!("/project/dir{}/file{}.rs", dir, file));
                tree.record_activity(&path, Some(AgentId::from_u128(1)));
            }
            tree.refresh_caches();
            let recent = tree.recent_activity(10);
            black_box(&recent);
            let visible = tree.visible_entries();
            black_box(&visible);
        });
    });
}

// -- Full Integration --

fn bench_full_event_to_render_cycle(c: &mut Criterion) {
    let (agents, focus, file_tree, terms) = make_app_state(6, 500);
    let grid = Grid::default();
    let mut widgets = WidgetRegistry::new();
    let area = Rect::new(0, 0, 200, 60);

    c.bench_function("full_event_to_render_6_agents_500_files", |b| {
        b.iter(|| {
            let focus_copy = &focus;

            let cell_rects = grid.compute_rects(area);

            for cell in &cell_rects {
                let r = &cell.rect;
                if 100 >= r.x && 100 < r.x + r.width && 30 >= r.y && 30 < r.y + r.height {
                    black_box(&cell.widget);
                    break;
                }
            }

            let backend = TestBackend::new(200, 60);
            let mut terminal = Terminal::new(backend).unwrap();

            terminal
                .draw(|frame| {
                    let render_term_fn =
                        |_id: AgentId, _buf: &mut ratatui::buffer::Buffer, _area: Rect| {};
                    let turns = auriga_core::TurnStore::new();
                    let traces = auriga_core::TraceStore::new();
                    let ctx = RenderContext {
                        agents: &agents,
                        turns: &turns,
                        traces: &traces,
                        focus: focus_copy,
                        file_tree: &file_tree,
                        render_term: &render_term_fn,
                        hidden_pages: &[],
                    };

                    for cell_rect in &cell_rects {
                        let widget = widgets.get_mut(cell_rect.widget);
                        widget.render(frame, cell_rect.rect, &ctx);
                    }
                })
                .unwrap();

            black_box(&terminal);
        });
    });
}

fn bench_worst_case_render(c: &mut Criterion) {
    let (agents, focus, file_tree, terms) = make_app_state(6, 5000);
    let grid = Grid::default();
    let mut widgets = WidgetRegistry::new();
    let area = Rect::new(0, 0, 300, 80);

    c.bench_function("worst_case_render_6_agents_5000_files_300x80", |b| {
        b.iter(|| {
            let backend = TestBackend::new(300, 80);
            let mut terminal = Terminal::new(backend).unwrap();

            terminal
                .draw(|frame| {
                    let cell_rects = grid.compute_rects(area);

                    let render_term_fn =
                        |_id: AgentId, _buf: &mut ratatui::buffer::Buffer, _area: Rect| {};
                    let turns = auriga_core::TurnStore::new();
                    let traces = auriga_core::TraceStore::new();
                    let ctx = RenderContext {
                        agents: &agents,
                        turns: &turns,
                        traces: &traces,
                        focus: &focus,
                        file_tree: &file_tree,
                        render_term: &render_term_fn,
                        hidden_pages: &[],
                    };

                    for cell_rect in &cell_rects {
                        let widget = widgets.get_mut(cell_rect.widget);
                        widget.render(frame, cell_rect.rect, &ctx);
                    }
                })
                .unwrap();

            black_box(&terminal);
        });
    });
}

criterion_group!(
    benches,
    bench_key_event_dispatch,
    bench_mouse_hit_test,
    bench_widget_click_dispatch,
    bench_agent_store_create_remove_churn,
    bench_file_tree_record_activity_churn,
    bench_file_tree_insert_new_files,
    bench_pty_output_processing,
    bench_file_event_burst,
    bench_full_event_to_render_cycle,
    bench_worst_case_render,
);
criterion_main!(benches);
