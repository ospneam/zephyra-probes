//! 文字渲染长稳（soak）压测应用。
//!
//! 用 `CpuHost` 无窗口驱动文字栈：它同步给出 `StageTimes` 与 `AtlasStats`，
//! 便于在 8–24 小时运行里观察内存增长、图集淘汰与帧耗时漂移。
//! 每 `phase-frames` 帧轮换一个压力场景（高级特性 / 密集 CJK / 彩色 emoji /
//! 装饰线+LCD 切换），每 `log-every` 帧向 stderr 打一行 CSV 风格指标。
//!
//! 默认系统字体，默认 60 FPS（可用 --fps 调整）。调试桥在此应用中不需要。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zephyra_core::context::Context;
use zephyra_core::core::Color;
use zephyra_core::element::Element;
use zephyra_core::id::NodeTag;
use zephyra_core::layout::Direction;
use zephyra_core::property::PropertyValue;
use zephyra_core::props;
use zephyra_core::text::{LcdOrder, TextDecoration, TextDecorationStyle};
use zephyra_fx::cpu_only::CpuHost;
use zephyra_fx::View;

/// FNV-1a：给动态生成的文本造一个稳定的 ElementKey（trait 只接受 &'static str / u64）。
fn key_of(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

struct SoakView {
    frame: Arc<AtomicU64>,
    phase_frames: u64,
}

impl SoakView {
    fn frame(&self) -> u64 {
        self.frame.load(Ordering::Relaxed)
    }

    fn phase(&self) -> u64 {
        (self.frame() / self.phase_frames) % 4
    }

    /// LCD 在第 0/2 个 phase 开，1/3 关。
    fn lcd_on(&self) -> bool {
        (self.frame() / self.phase_frames) % 2 == 0
    }
}

impl View for SoakView {
    fn on_input(&mut self, _state: &zephyra_fx::InputState) {}

    fn view(&self, _ctx: &Context<'_>) -> Element {
        let mut rows = match self.phase() {
            0 => advanced_rows(self.lcd_on()),
            1 => cjk_rows(),
            2 => emoji_rows(),
            _ => decoration_rows(self.lcd_on()),
        };
        // 持续变化的旋转：强制每帧都真正重绘（同一组字形，不会让图集无界增长）。
        // 否则同一 phase 内内容不变，FrameCache 会跳过 paint，压测强度大打折扣。
        rows.push(text_row("旋转探针：持续变化以强制每帧重绘".into(), 15.0, |e| {
            let a = (self.frame() as f32 * 0.02).sin() * 0.3;
            e.with(&props::TEXT_ROTATION, PropertyValue::float(a))
        }));
        Element::new(NodeTag::Box)
            .with_key("soak-root")
            .with(
                &props::DIRECTION,
                PropertyValue::direction(Direction::Vertical),
            )
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
                    .with(&props::TEXT_ROTATION, PropertyValue::float(0.02 * i as f32))
                    .with(
                        &props::TEXT_SHADOW,
                        PropertyValue::custom(zephyra_core::text::TextShadow {
                            color: Color::rgba(0.0, 0.0, 0.0, 0.45),
                            offset: zephyra_core::core::Point::new(1.0, 1.0),
                            blur_sigma: 1.0,
                        }),
                    );
                if lcd_on {
                    e.with(&props::TEXT_LCD, PropertyValue::custom(LcdOrder::Rgb))
                } else {
                    e
                }
            })
        })
        .collect()
}

fn memory_kb() -> (u64, u64) {
    #[cfg(windows)]
    {
        use windows::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;
        unsafe {
            let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            if GetProcessMemoryInfo(
                GetCurrentProcess(),
                &mut c,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
            .is_ok()
            {
                return ((c.WorkingSetSize as u64) / 1024, (c.PagefileUsage as u64) / 1024);
            }
        }
    }
    (0, 0)
}

fn percentile(samples: &mut VecDeque<u128>, p: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let mut s: Vec<u128> = samples.iter().copied().collect();
    s.sort_unstable();
    s[(s.len() * p / 100).min(s.len() - 1)]
}

fn arg_usize(name: &str, default: usize) -> usize {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn main() {
    let phase_frames = arg_usize("--phase-frames", 300).max(1) as u64;
    let log_every = arg_usize("--log-every", 300).max(1) as u64;
    let fps = arg_usize("--fps", 60).max(1) as f64;
    let duration_secs = arg_usize("--duration-secs", 0) as f64;
    let warmup = arg_usize("--warmup-frames", 20) as u64;
    let interval = Duration::from_secs_f64(1.0 / fps);

    let frame_counter = Arc::new(AtomicU64::new(0));
    let view = SoakView {
        frame: frame_counter.clone(),
        phase_frames,
    };
    let mut host = CpuHost::new(view, (1100, 760));

    let started = Instant::now();
    let mut last_log = started;
    let mut last_log_frame = 0u64;
    let mut samples: VecDeque<u128> = VecDeque::with_capacity(512);
    let mut frame: u64 = 0;

    eprintln!(
        "[soak] 启动：phase_frames={phase_frames} log_every={log_every} fps={fps:.0} duration={:.0}s",
        duration_secs
    );

    loop {
        frame += 1;
        frame_counter.store(frame, Ordering::Relaxed);

        let (f, times) = host.frame_timed();
        let frame_us = times.total_us as u128;

        // 预热帧（首帧栅格化全部字形、ATL 瞬间填充）不进 p95，否则会永久污染分位数。
        if f.redrawn && frame > warmup {
            samples.push_back(frame_us);
            if samples.len() > 512 {
                samples.pop_front();
            }
        }

        if frame - last_log_frame >= log_every {
            let elapsed = started.elapsed();
            let dt = last_log.elapsed();
            let df = frame - last_log_frame;
            let inst_fps = if dt.as_secs_f64() > 0.0 {
                df as f64 / dt.as_secs_f64()
            } else {
                0.0
            };
            let (ws, pf) = memory_kb();
            let stats = host.atlas_stats();
            let p95 = percentile(&mut samples, 95);
            eprintln!(
                "[soak] t={:.1}s frame={} fps={:.1} frame_us={} paint_us={} expand_us={} p95_us={} quads={} glyphs={} evictions={} atlas_used_px={} ws_kb={} pf_kb={}",
                elapsed.as_secs_f64(),
                frame,
                inst_fps,
                frame_us,
                times.paint_us,
                times.expand_us,
                p95,
                f.render.quads,
                stats.glyphs,
                stats.evictions,
                stats.used_px,
                ws,
                pf
            );
            last_log = Instant::now();
            last_log_frame = frame;
        }

        if duration_secs > 0.0 && started.elapsed().as_secs_f64() >= duration_secs {
            eprintln!("[soak] 到达 --duration-secs={duration_secs}s，退出");
            break;
        }

        // 节流到目标 FPS；真实帧成本已在 frame_us 中记录（不含 sleep）。
        std::thread::sleep(interval);
    }
}
