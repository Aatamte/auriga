use auriga_grid::Grid;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ratatui::layout::Rect;

fn bench_grid_compute_rects(c: &mut Criterion) {
    let grid = Grid::default();
    let area = Rect::new(0, 0, 200, 60);

    c.bench_function("grid_compute_rects", |b| {
        b.iter(|| {
            black_box(grid.compute_rects(black_box(area)));
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

criterion_group!(benches, bench_grid_compute_rects, bench_mouse_hit_test,);
criterion_main!(benches);
