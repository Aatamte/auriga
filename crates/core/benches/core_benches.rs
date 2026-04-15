use auriga_core::{
    AgentId, AgentStore, FileEntry, FileTree, FocusState, ScrollDirection, Scrollable,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
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

criterion_group!(
    benches,
    bench_file_tree_visible_entries,
    bench_file_tree_visible_large,
    bench_scrollable_operations,
    bench_key_event_dispatch,
    bench_agent_store_create_remove_churn,
    bench_file_tree_record_activity_churn,
    bench_file_tree_insert_new_files,
    bench_file_event_burst,
);
criterion_main!(benches);
