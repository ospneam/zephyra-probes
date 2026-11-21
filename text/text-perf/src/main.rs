//! 文本渲染性能基准（headless，跑固定帧数出报告）。
//!
//! 用 CpuHost 驱动与 text-soak 相同的 4 个压力场景，收集 `StageTimes` 的
//! total / paint / expand 三列，输出 min/avg/p50/p95/p99/max 与吞吐。
//! `--assert-p95-us N` 可在 p95 超阈值时 panic（CI 防性能回退）。
//!
//! 默认系统字体。与长稳 soak 分开：这里只测速度，不测内存漂移。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use zephyra_core::context::Context;
use zephyra_core::core::Color;
use zephyra_core::element::Element;
use zephyra_core::id::NodeTag;
use zephyra_core::layout::Direction;
use zephyra_core::property::PropertyValue;
use zephyra_core::props;
use zephyra_core::text::{LcdOrder, TextDecoration, TextDecorationStyle};
use zephyra_fx::cpu_only::{CpuHost, StageTimes};
use zephyra_fx::View;

fn key_of(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

struct PerfView {
    frame: Arc<AtomicU64>,
    phase_frames: u64,
}

impl View for PerfView {
    fn on_input(&mut self, _state: &zephyra_fx::InputState) {}

    fn view(&self, _ctx: &Context<'_>) -> Element {
        let f = self.frame.load(Ordering::Relaxed);
        let phase = (f / self.phase_frames) % 4;
        let lcd_on = (f / self.phase_frames) % 2 == 0;
        let mut rows = match phase {
            0 => advanced_rows(lcd_on),
            1 => cjk_rows(),
            2 => emoji_rows(),
            _ => decoration_rows(lcd_on),
        };
        // 持续变化的角度，强制每帧真重绘（同一组字形，不撑大图集）。
        rows.push(text_row("旋转探针：强制每帧重绘".into(), 15.0, |e| {
            let a = (f as f32 * 0.02).sin() * 0.3;
            e.with(&props::TEXT_ROTATION, PropertyValue::float(a))
        }));
        Element::new(NodeTag::Box)
            .with_key("perf-root")
            .with(&props::DIRECTION, PropertyValue::direction(Direction::Vertical))
            .with(&props::SPACING, PropertyValue::float(6.0))
            .with(
                &props::BACKGROUND,
                PropertyValue::color(Color::rgb(0.06, 0.07, 0.09)),
            )
            .with_children(rows)
    }
}

fn text_row(text: String, size: f32, decorate: impl FnOnce(Element) -> Element) -> Element {
    let base = Element::new(NodeTag::Text)
        .with_key(key_of(&text))
        .with(&props::TEXT, PropertyValue::Str(text.into()))
        .with(&props::TEXT_SIZE, PropertyValue::float(size))
        .with(
            &props::TEXT_COLOR,
            PropertyValue::color(Color::rgb(0.95, 0.95, 0.98)),
        )
        .with(&props::TEXT_MAX_WIDTH, PropertyValue::float(1040.0));
    decorate(base)
}

fn advanced_rows(lcd_on: bool) -> Vec<Element> {
    let mut v = vec![text_row("场景 0/3：高级文字特性全开".into(), 26.0, |e| e)];
    for i in 0..8 {
        let size = 12.0 + i as f32 * 2.0;
        let text = format!(
            "第 {i} 行：字距/词距、装饰线、阴影、连字、可变轴、CJK 与 Latin 混排 0123456789"
        );
        v.push(text_row(text, size, |e| {
            let e = e
                .with(&props::TEXT_LETTER_SPACING, PropertyValue::float(1.0))
                .with(&props::TEXT_WORD_SPACING, PropertyValue::float(3.0))
                .with(
                    &props::TEXT_SHADOW,
                    PropertyValue::custom(zephyra_core::text::TextShadow {
                        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
                        offset: zephyra_core::core::Point::new(1.0, 2.0),
                        blur_sigma: 1.0,
                    }),
                )
                .with(
                    &props::FONT_FEATURES,
                    PropertyValue::custom(vec![(u32::from_be_bytes(*b"tnum"), 1u32)]),
                )
                .with(
                    &props::TEXT_VARIATIONS,
                    PropertyValue::custom(vec![(*b"wght", 500.0f32 + i as f32 * 40.0)]),
                );
            if lcd_on {
                e.with(&props::TEXT_LCD, PropertyValue::custom(LcdOrder::Rgb))
            } else {
                e
            }
        }));
    }
    v
}

fn cjk_rows() -> Vec<Element> {
    let base = "中文排版压力测试：字形整形、字距、行高、断行与图集淘汰。";
    (0..16)
        .map(|i| {
            let size = 11.0 + (i % 8) as f32 * 2.5;
            let text = format!("{i:02} {base}{base}");
            text_row(text, size, |e| {
                e.with(&props::TEXT_MAX_LINES, PropertyValue::float(2.0))
            })
        })
        .collect()
}

fn emoji_rows() -> Vec<Element> {
    let one = "😀🚀🎉❤️👍🏽🌈🔥⭐";
    (0..12)
        .map(|i| {
            let size = 14.0 + (i % 5) as f32 * 4.0;
            let text = format!("行 {i:02} {one}{one}{one}");
            text_row(text, size, |e| e)
        })
        .collect()
}

fn decoration_rows(lcd_on: bool) -> Vec<Element> {
    (0..14)
        .map(|i| {
            let size = 13.0 + (i % 4) as f32 * 3.0;
            let text = format!("装饰线与阴影压力 {i:02}：underline / wavy / gaps / rotation");
            text_row(text, size, |e| {
                let e = e
                    .with(
                        &props::TEXT_DECORATION,
                        PropertyValue::custom(TextDecoration {
                            underline: true,
                            ..Default::default()
                        }),
                    )
                    .with(
                        &props::TEXT_DECORATION_STYLE,
                        PropertyValue::custom(TextDecorationStyle::Wavy),
                    )
                    .with(&props::TEXT_ROTATION, PropertyValue::float(0.02 * i as f32));
                if lcd_on {
                    e.with(&props::TEXT_LCD, PropertyValue::custom(LcdOrder::Rgb))
                } else {
                    e
                }
            })
        })
        .collect()
}

/// 一列样本的统计。
struct Column {
    name: &'static str,
    samples: Vec<u64>,
}

impl Column {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            samples: Vec::new(),
        }
    }
    fn push(&mut self, v: u64) {
        self.samples.push(v);
    }
    fn report(&self) -> String {
        if self.samples.is_empty() {
            return format!("{} n=0", self.name);
        }
        let mut s = self.samples.clone();
        s.sort_unstable();
        let n = s.len();
        let q = |p: usize| s[((n - 1) * p).div_ceil(100)];
        let sum: u64 = s.iter().sum();
        format!(
            "{} n={} min={} avg={:.1} p50={} p95={} p99={} max={}",
            self.name,
            n,
            s[0],
            sum as f64 / n as f64,
            q(50),
            q(95),
            q(99),
            s[n - 1]
        )
    }
    fn p95(&self) -> u64 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut s = self.samples.clone();
        s.sort_unstable();
        s[((s.len() - 1) * 95).div_ceil(100)]
    }
}

fn arg_usize(name: &str, default: usize) -> usize {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn main() {
    let frames = arg_usize("--frames", 600).max(1);
    let warmup = arg_usize("--warmup", 60);
    let phase_frames = arg_usize("--phase-frames", 150).max(1) as u64;
    let assert_p95: Option<u64> = {
        let args: Vec<String> = std::env::args().collect();
        args.windows(2)
            .find(|w| w[0] == "--assert-p95-us")
            .and_then(|w| w[1].parse().ok())
    };

    let counter = Arc::new(AtomicU64::new(0));
    let view = PerfView {
        frame: counter.clone(),
        phase_frames,
    };
    let mut host = CpuHost::new(view, (1100, 760));

    let mut total = Column::new("total_us");
    let mut paint = Column::new("paint_us");
    let mut expand = Column::new("expand_us");
    let mut view_col = Column::new("view_us");

    eprintln!(
        "[text-perf] 启动：frames={frames} warmup={warmup} phase_frames={phase_frames} assert_p95_us={:?}",
        assert_p95
    );

    let started = Instant::now();
    for i in 1..=frames {
        counter.store(i as u64, Ordering::Relaxed);
        let (_f, t): (_, StageTimes) = host.frame_timed();
        if i > warmup {
            total.push(t.total_us as u64);
            paint.push(t.paint_us as u64);
            expand.push(t.expand_us as u64);
            view_col.push(t.view_us as u64);
        }
    }
    let elapsed = started.elapsed();

    println!("PROFILE text-perf");
    println!("frames={} warmup={} elapsed_ms={}", frames, warmup, elapsed.as_millis());
    let measured = (frames - warmup.min(frames)) as f64;
    if elapsed.as_secs_f64() > 0.0 {
        println!("throughput_fps={:.1}", measured / elapsed.as_secs_f64());
    }
    println!("{}", total.report());
    println!("{}", paint.report());
    println!("{}", expand.report());
    println!("{}", view_col.report());

    if let Some(limit) = assert_p95 {
        let p95 = total.p95();
        assert!(
            p95 <= limit,
            "[text-perf] 断言失败：total p95 {p95}µs > 阈值 {limit}µs"
        );
        println!("assert_p95_us={limit} p95={p95} PASS");
    }
}
