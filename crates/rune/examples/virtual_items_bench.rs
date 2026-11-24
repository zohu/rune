//! Paired headless measurements. Run with the perf profile; no GPU upload or presentation.
use rune::{Color, Cx, El, Env, Live, Runtime, VirtualItems, col, el, text};
use std::{cell::Cell, rc::Rc, time::Instant};

struct Source {
    catalog: Vec<String>,
    offset: Cell<usize>,
    revision: Cell<u64>,
    builds: Cell<usize>,
    complex: bool,
    opacity: Live<f32>,
}
impl VirtualItems for Source {
    fn version(&self) -> u64 {
        self.revision.get()
    }
    fn keys(&self) -> Vec<String> {
        (0..50).map(|i| self.catalog[(self.offset.get() + i) % self.catalog.len()].clone()).collect()
    }
    fn build(&self, key: &str) -> El {
        self.builds.set(self.builds.get() + 1);
        let i = key[1..].parse::<usize>().unwrap();
        let item = col().absolute((i % 10) as f32 * 60.0, (i % 5) as f32 * 60.0).size(54.0, 54.0);
        if !self.complex {
            return item.fill(Color::WHITE);
        }
        item.live_opacity(&self.opacity).children((0..19).map(|j| {
            let p =
                if j % 4 == 0 { text("Canvas node").h(5.0) } else { el().size(8.0, 2.0).fill(Color::WHITE) };
            if j % 3 == 0 { p.live_opacity(&self.opacity) } else { p }
        }))
    }
}
struct App {
    source: Rc<Source>,
    views: Cell<usize>,
}
fn view(s: &App, _: &mut Cx<App>) -> El {
    s.views.set(s.views.get() + 1);
    el().size(640.0, 480.0).virtual_items(s.source.clone())
}

fn main() {
    assert!(!cfg!(debug_assertions), "benchmarks require --profile perf");
    println!("BENCH,kind,total,enter,full,round,mean_ms,p95_ms,max_ms,views,builds");
    for complex in [false, true] {
        for total in [100, 10_000, 50_000] {
            for enter in [1, 4, 16] {
                for round in 0..5 {
                    for full in if round % 2 == 0 { [false, true] } else { [true, false] } {
                        let source = Rc::new(Source {
                            catalog: (0..total).map(|i| format!("n{i}")).collect(),
                            offset: Cell::new(0),
                            revision: Cell::new(0),
                            builds: Cell::new(0),
                            complex,
                            opacity: Live::new(0.8),
                        });
                        let mut rt = Runtime::new(
                            App { source: source.clone(), views: Cell::new(0) },
                            view,
                            (640.0, 480.0),
                            Env::new(true, 2.0),
                        );
                        rt.frame(0.0);
                        let mut samples = Vec::new();
                        for frame in 0..420 {
                            let start = Instant::now();
                            source.offset.set((source.offset.get() + enter) % total);
                            source.revision.set(source.revision.get() + 1);
                            let views = rt.state.views.get();
                            let builds = source.builds.get();
                            if full {
                                rt.invalidate();
                            }
                            rt.frame(1.0 / 120.0);
                            let ms = start.elapsed().as_secs_f64() * 1000.0;
                            assert_eq!(rt.state.views.get() - views, usize::from(full));
                            assert_eq!(source.builds.get() - builds, if full { 50 } else { enter });
                            assert_eq!(
                                rt.elements().iter().filter(|n| n.path.matches('/').count() == 1).count(),
                                50
                            );
                            if frame >= 120 {
                                samples.push(ms);
                            }
                        }
                        samples.sort_by(f64::total_cmp);
                        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
                        println!(
                            "BENCH,{},{total},{enter},{full},{round},{mean:.6},{:.6},{:.6},{},{}",
                            if complex { "complex" } else { "simple" },
                            samples[284],
                            samples[299],
                            if full { 300 } else { 0 },
                            if full { 15_000 } else { enter * 300 }
                        );
                    }
                }
            }
        }
    }
}
