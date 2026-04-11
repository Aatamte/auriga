use auriga_core::{
    AgentId, AgentStore, FileEntry, FileTree, FocusState, ScrollDirection, Scrollable,
};
use auriga_grid::Grid;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
// grid_to_lines was replaced by render_term
use auriga_widgets::{RenderContext, WidgetRegistry};
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

fn bench_grid_compute_rects(c: &mut Criterion) {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 200, 60);

    c.bench_function("grid_compute_rects", |b| {
        b.iter(|| {
            black_box(grid.compute_rects(black_box(area)));
        });
    });
}

// bench_grid_to_lines_small and bench_grid_to_lines_large removed — grid_to_lines was replaced by render_term

fn bench_file_tree_visible_entries(c: &mut Criterion) {
    let mut tree = make_file_tree(500);
    tree.refresh_caches();

    c.bench_function("file_tree_visible_500_entries", |b| {
        b.iter(|| {
            black_box(tree.visible_entries());
        });
    });
}

fn bench_file_tree_visible_large(c: &mut Criterion) {
    let mut tree = make_file_tree(5000);
    tree.refresh_caches();

    c.bench_function("file_tree_visible_5000_entries", |b| {
        b.iter(|| {
            black_box(tree.visible_entries());
        });
    });
}

fn bench_scrollable_operations(c: &mut Criterion) {
    c.bench_function("scrollable_1000_scroll_ops", |b| {
        b.iter(|| {
            let mut s = Scrollable::new();
            s.set_item_count(1000);
            s.set_visible_height(40);
            for _ in 0..500 {
                s.scroll(ScrollDirection::Down);
            }
            for _ in 0..500 {
                s.scroll(ScrollDirection::Up);
            }
            black_box(&s);
        });
    });
}

fn bench_full_render_cycle(c: &mut Criterion) {
    let (agents, agent_ids) = make_agents(5);
    let focus = {
        let mut f = FocusState::new();
        f.set_active_agent(agent_ids[0]);
        f
    };
    let mut file_tree = make_file_tree(200);
    file_tree.refresh_caches();
    let grid = Grid::default();
    let mut widgets = WidgetRegistry::new();

    let terms: Vec<Term<EventProxy>> = (0..5).map(|_| make_term_with_output(40, 120)).collect();

    let area = Rect::new(0, 0, 200, 60);

    c.bench_function("full_render_cycle_5_agents", |b| {
        b.iter(|| {
            let backend = TestBackend::new(200, 60);
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

fn bench_full_render_cycle_many_agents(c: &mut Criterion) {
    let (agents, agent_ids) = make_agents(20);
    let focus = {
        let mut f = FocusState::new();
        f.set_active_agent(agent_ids[0]);
        f
    };
    let mut file_tree = make_file_tree(1000);
    file_tree.refresh_caches();
    let grid = Grid::default();
    let mut widgets = WidgetRegistry::new();

    let terms: Vec<Term<EventProxy>> = (0..20).map(|_| make_term_with_output(40, 120)).collect();

    let area = Rect::new(0, 0, 200, 60);

    c.bench_function("full_render_cycle_20_agents", |b| {
        b.iter(|| {
            let backend = TestBackend::new(200, 60);
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
    bench_grid_compute_rects,
    bench_file_tree_visible_entries,
    bench_file_tree_visible_large,
    bench_scrollable_operations,
    bench_full_render_cycle,
    bench_full_render_cycle_many_agents,
);
criterion_main!(benches);
