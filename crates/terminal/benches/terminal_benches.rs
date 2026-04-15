use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::vte;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

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

fn bench_pty_output_processing(c: &mut Criterion) {
    c.bench_function("alacritty_process_1mb_output", |b| {
        let config = TermConfig {
            scrolling_history: 1000,
            ..Default::default()
        };
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

criterion_group!(benches, bench_pty_output_processing,);
criterion_main!(benches);
