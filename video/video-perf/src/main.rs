//! 视频解码性能基准（headless，跑固定帧数出报告）。
//!
//! 反复 open → 解码，对每一帧测量「poll_video_frame + import_video_frame」的
//! 消费者侧耗时与帧间隔，输出 min/avg/p50/p95/p99/max 与吞吐。
//! `--assert-p95-us N` 在 poll+import 的 p95 超阈值时 panic。
//!
//! 注意：解码在 media 的后台线程进行，本基准测的是主线程拿到帧并转成
//! `FrameDesc` 的延迟，以及帧到帧的节奏；两者结合反映端到端解码体验。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use zephyra_media::{import_video_frame, MediaPlayer, PlayerOptions};

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

fn arg_f64(name: &str, default: f64) -> f64 {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn arg_path(name: &str) -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| PathBuf::from(&w[1]))
}

fn main() {
    let path = arg_path("--path").expect("缺少 --path：cargo run -- --path <测试.mp4>");
    if !path.exists() {
        panic!("视频文件不存在：{}", path.display());
    }
    let frames = arg_usize("--frames", 500).max(1);
    let warmup = arg_usize("--warmup", 20);
    let session_idle = arg_f64("--session-idle-secs", 1.0).max(0.2);
    let max_secs = arg_f64("--max-seconds", 120.0);
    let assert_p95: Option<u64> = {
        let args: Vec<String> = std::env::args().collect();
        args.windows(2)
            .find(|w| w[0] == "--assert-p95-us")
            .and_then(|w| w[1].parse().ok())
    };

    let mut poll_import = Column::new("poll_import_us");
    let mut interval = Column::new("interval_us");

    eprintln!(
        "[video-perf] 启动：path={} frames={frames} warmup={warmup} assert_p95_us={:?}",
        path.display(),
        assert_p95
    );

    let started = Instant::now();
    let mut delivered = 0usize;
    let mut session_frames = 0usize;
    let mut last_frame_at = started;
    let mut prev_delivered_at: Option<Instant> = None;
    let mut player = MediaPlayer::open(&path, PlayerOptions::default())
        .unwrap_or_else(|e| panic!("打开视频失败：{e}"));

    while delivered < frames && started.elapsed().as_secs_f64() < max_secs {
        let t0 = Instant::now();
        if let Some(frame) = player.poll_video_frame() {
            // 消费者侧：poll 拿到帧后转成 FrameDesc 的耗时。
            let _ = import_video_frame(&frame);
            let cost = t0.elapsed().as_micros() as u64;
            let now = Instant::now();
            delivered += 1;
            session_frames += 1;
            last_frame_at = now;
            if delivered > warmup {
                poll_import.push(cost);
                if let Some(prev) = prev_delivered_at {
                    interval.push(now.duration_since(prev).as_micros() as u64);
                }
            }
            prev_delivered_at = Some(now);
        } else {
            // 推进音频时钟，避免 presentation clock 卡住。
            let _ = player.poll_audio_frame();
            if session_frames > 0 && last_frame_at.elapsed().as_secs_f64() >= session_idle {
                // 本片解码完：reopen 继续采样（一次性载入模型）。
                player = MediaPlayer::open(&path, PlayerOptions::default())
                    .unwrap_or_else(|e| panic!("reopen 失败：{e}"));
                session_frames = 0;
                // 会话切换的间隔属于"重新打开文件"，不是帧节奏，排除出 interval 样本。
                prev_delivered_at = None;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    let elapsed = started.elapsed();
    let measured = delivered.saturating_sub(warmup.min(delivered)) as f64;
    println!("PROFILE video-perf");
    println!(
        "path={} delivered={} warmup={} elapsed_ms={}",
        path.display(),
        delivered,
        warmup,
        elapsed.as_millis()
    );
    if elapsed.as_secs_f64() > 0.0 {
        println!("throughput_fps={:.1}", measured / elapsed.as_secs_f64());
    }
    println!("{}", poll_import.report());
    println!("{}", interval.report());

    assert!(
        delivered > warmup,
        "[video-perf] 断言失败：只采到 {delivered} 帧（warmup={warmup}），样本不足"
    );
    if let Some(limit) = assert_p95 {
        let p95 = poll_import.p95();
        assert!(
            p95 <= limit,
            "[video-perf] 断言失败：poll_import p95 {p95}µs > 阈值 {limit}µs"
        );
        println!("assert_p95_us={limit} p95={p95} PASS");
    }
}
