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

fn percentile(samples: &VecDeque<u128>, p: usize) -> u128 {
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

fn arg_u64(name: &str, default: u64) -> u64 {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn arg_f64(name: &str, default: f64) -> f64 {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn arg_flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// p95 断言至少需要这么多 per-phase 样本才有统计意义。
const MIN_P95_SAMPLES: usize = 24;

/// soak 运行期断言：任意一项不达标立即 panic（非零退出码，便于 CI 判定）。
///
/// 基线在预热期结束后建立，之后每个日志点都做增量检查：
/// - 工作集 / pagefile 相对预热基线增长不超过 `memory_pct`%
/// - 图集淘汰数不超过 `evictions_max`（稳态应为 0）
/// - 同一 phase 的 p95 不比该 phase 基线漂移超过 `p95_drift_pct`%
/// - 同一 phase 的 quad 数变化不超过 `quad_drift`
///
/// p95 / quad 按 phase 分别比较：emoji 场景的 COLR 合成天然比文本场景慢一个
/// 数量级，混在一起比会产生大量误报。
struct AssertState {
    enabled: bool,
    memory_pct: f64,
    /// 基线低于此值（KB）时跳过百分比判定，避免小基数抖动误报。
    memory_floor_kb: u64,
    evictions_max: u64,
    p95_drift_pct: f64,
    /// p95 基线低于此值（µs）时不判定：样本小、抖动大，百分比无意义。
    p95_floor_us: u128,
    /// 连续 N 次超阈值才 panic（消除单样本尖峰误报）。
    p95_strikes_required: u32,
    quad_drift: usize,
    warmup_secs: f64,
    baseline_ws: Option<u64>,
    baseline_pf: Option<u64>,
    phase_p95: [Option<u128>; 4],
    /// 各 phase 连续超阈值的次数（达到 p95_strikes_required 才 panic）。
    phase_p95_strikes: [u32; 4],
    phase_quads: [Option<usize>; 4],
}

impl AssertState {
    fn from_args() -> Self {
        Self {
            enabled: !arg_flag("--no-assert"),
            memory_pct: arg_f64("--assert-memory-pct", 10.0),
            memory_floor_kb: arg_u64("--assert-memory-floor-kb", 64 * 1024),
            evictions_max: arg_usize("--assert-evictions", 0) as u64,
            p95_drift_pct: arg_f64("--assert-p95-drift-pct", 50.0),
            p95_floor_us: arg_u64("--assert-p95-floor-us", 2000) as u128,
            p95_strikes_required: arg_usize("--assert-p95-strikes", 3) as u32,
            quad_drift: arg_usize("--assert-quad-drift", 0),
            warmup_secs: arg_f64("--assert-warmup-secs", 10.0),
            baseline_ws: None,
            baseline_pf: None,
            phase_p95: [None; 4],
            phase_p95_strikes: [0; 4],
            phase_quads: [None; 4],
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check(
        &mut self,
        elapsed_secs: f64,
        phase: usize,
        ws: u64,
        pf: u64,
        evictions: u64,
        phase_p95: u128,
        p95_samples: usize,
        quads: usize,
    ) {
        if !self.enabled || elapsed_secs < self.warmup_secs {
            return;
        }

        // 内存：预热后首个采样作基线。非 Windows 上 ws/pf 恒为 0，跳过。
        if ws > 0 && pf > 0 {
            match (self.baseline_ws, self.baseline_pf) {
                (None, _) => {
                    self.baseline_ws = Some(ws);
                    self.baseline_pf = Some(pf);
                }
                (Some(bw), Some(bp)) => {
                    let ws_growth = (ws as f64 / bw as f64 - 1.0) * 100.0;
                    let pf_growth = (pf as f64 / bp as f64 - 1.0) * 100.0;
                    if bw >= self.memory_floor_kb {
                        assert!(
                            ws_growth <= self.memory_pct,
                            "[soak] 断言失败：工作集 {ws} KB 相对预热基线 {bw} KB 增长 {ws_growth:.1}% > {:.1}%",
                            self.memory_pct
                        );
                    }
                    if bp >= self.memory_floor_kb {
                        assert!(
                            pf_growth <= self.memory_pct,
                            "[soak] 断言失败：pagefile {pf} KB 相对预热基线 {bp} KB 增长 {pf_growth:.1}% > {:.1}%",
                            self.memory_pct
                        );
                    }
                }
                _ => {}
            }
        }

        assert!(
            evictions <= self.evictions_max,
            "[soak] 断言失败：图集累计淘汰 {evictions} > 上限 {}",
            self.evictions_max
        );

        // quad 数：同一 phase 内容固定，不应变化。
        match self.phase_quads[phase] {
            None => self.phase_quads[phase] = Some(quads),
            Some(base) => {
                let drift = quads.abs_diff(base);
                assert!(
                    drift <= self.quad_drift,
                    "[soak] 断言失败：phase {phase} 的 quad 数从 {base} 变为 {quads}（漂移 {drift} > {}）",
                    self.quad_drift
                );
            }
        }

        // p95：同 phase 比较，样本不足时只记基线、不判定。
        if p95_samples >= MIN_P95_SAMPLES {
            match self.phase_p95[phase] {
                None => self.phase_p95[phase] = Some(phase_p95),
                Some(base) if base >= self.p95_floor_us => {
                    let drift = (phase_p95 as f64 / base as f64 - 1.0) * 100.0;
                    let strikes = &mut self.phase_p95_strikes[phase];
                    if drift > self.p95_drift_pct {
                        *strikes += 1;
                        assert!(
                            *strikes < self.p95_strikes_required,
                            "[soak] 断言失败：phase {phase} 的 p95 从 {base}µs 漂移至 {phase_p95}µs（+{drift:.1}% > {:.1}%），连续 {strikes} 次",
                            self.p95_drift_pct
                        );
                    } else {
                        // 回到阈值内就清零：只抓持续回归，不抓偶发尖峰。
                        *strikes = 0;
                    }
                }
                Some(_) => {}
            }
        }
    }
}

fn main() {
    let phase_frames = arg_usize("--phase-frames", 300).max(1) as u64;
    let log_every = arg_usize("--log-every", 300).max(1) as u64;
    let fps = arg_usize("--fps", 60).max(1) as f64;
    let duration_secs = arg_usize("--duration-secs", 0) as f64;
    let warmup = arg_usize("--warmup-frames", 20) as u64;
    let interval = Duration::from_secs_f64(1.0 / fps);
    let mut asserts = AssertState::from_args();

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
    // 每个 phase 独立样本：emoji 场景天然慢一个数量级，混算会让 p95 无法比较。
    let mut phase_samples: [VecDeque<u128>; 4] =
        std::array::from_fn(|_| VecDeque::with_capacity(256));
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
        let phase = ((frame / phase_frames) % 4) as usize;
        if f.redrawn && frame > warmup {
            samples.push_back(frame_us);
            if samples.len() > 512 {
                samples.pop_front();
            }
            let ps = &mut phase_samples[phase];
            ps.push_back(frame_us);
            if ps.len() > 256 {
                ps.pop_front();
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
            let p95 = percentile(&samples, 95);
            let phase_p95 = percentile(&phase_samples[phase], 95);
            eprintln!(
                "[soak] t={:.1}s frame={} phase={} fps={:.1} frame_us={} paint_us={} expand_us={} p95_us={} phase_p95={} strikes={} quads={} glyphs={} evictions={} atlas_used_px={} ws_kb={} pf_kb={}",
                elapsed.as_secs_f64(),
                frame,
                phase,
                inst_fps,
                frame_us,
                times.paint_us,
                times.expand_us,
                p95,
                phase_p95,
                asserts.phase_p95_strikes[phase],
                f.render.quads,
                stats.glyphs,
                stats.evictions,
                stats.used_px,
                ws,
                pf
            );
            asserts.check(
                elapsed.as_secs_f64(),
                phase,
                ws,
                pf,
                stats.evictions,
                phase_p95,
                phase_samples[phase].len(),
                f.render.quads,
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
