use auriga_core::{AgentId, AgentStore, FileEntry, FileTree, FocusState, TraceStore, TurnStore};
use auriga_widgets::{RenderContext, Widget, WidgetRegistry};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ratatui::layout::Rect;
use std::path::PathBuf;

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

fn bench_widget_click_dispatch(c: &mut Criterion) {
    let (agents, agent_ids) = make_agents(20);
    let mut focus = FocusState::new();
    focus.set_active_agent(agent_ids[0]);
    let mut file_tree = make_file_tree(500);
    file_tree.refresh_caches();
    let mut widgets = WidgetRegistry::new();

    c.bench_function("widget_click_agent_pane_20_agents", |b| {
        b.iter(|| {
            let render_term_fn = |_id: AgentId, _buf: &mut ratatui::buffer::Buffer, _area: Rect| {};
            let turns = TurnStore::new();
            let traces = TraceStore::new();
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
                black_box(widgets.agent_pane.handle_click(row, 0, &ctx));
            }
        });
    });
}

criterion_group!(benches, bench_widget_click_dispatch,);
criterion_main!(benches);
