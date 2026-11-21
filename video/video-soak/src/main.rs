//! 视频解码长稳（soak）压测应用（headless，无窗口）。
//!
//! 反复「打开文件 → 解码全部帧 → 关闭」，在运行期断言四项：
//!   1. 帧几何与格式：每帧的 coded_size / plane 数 / stride×rows / NV12 转换都合法。
//!   2. 内存无单调增长：预热后工作集 / pagefile 相对基线增幅不超阈值。
//!   3. 持续出帧不卡死：每个会话必须在超时内出帧，否则解码线程视为假死。
//!   4. 反复 open/drop 无泄漏：会话循环即 open/drop 压力，内存需稳定。
//!
//! 用法：cargo run -- --path <你的测试.mp4> --duration-secs 86400
//! 默认关闭断言用 --no-assert（只打日志）。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use zephyra_media::{import_video_frame, MediaPlayer, PlayerOptions, VideoFrameSource};
use zephyra_render_ext::prelude::ExternalTextureFormat;

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

fn arg_f64(name: &str, default: f64) -> f64 {
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

fn arg_path(name: &str) -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| PathBuf::from(&w[1]))
}

fn main() {
    let path = arg_path("--path").expect(
        "缺少 --path：请把测试用 MP4 放到本地并传入，例如 --path E:\\videos\\test.mp4",
    );
    if !path.exists() {
        panic!("视频文件不存在：{}", path.display());
    }

    let duration_secs = arg_f64("--duration-secs", 60.0);
    let log_every_secs = arg_f64("--log-every", 5.0).max(0.1);
    let warmup_secs = arg_f64("--assert-warmup-secs", 5.0);
    let memory_pct = arg_f64("--assert-memory-pct", 10.0);
    // 绝对下限：基线很小时（如刚启动的 pagefile 只有几 MB），百分比会被
    // 几百 KB 的抖动放大成 30%+ 造成误报。低于此基线不按百分比判定。
    let memory_floor_kb = arg_u64("--assert-memory-floor-kb", 64 * 1024);
    let stall_secs = arg_f64("--stall-timeout-secs", 5.0).max(0.5);
    let session_idle_secs = arg_f64("--session-idle-secs", 1.5).max(0.2);
    let enabled = !std::env::args().any(|a| a == "--no-assert");

    eprintln!(
        "[video-soak] 启动：path={} duration={:.0}s log_every={:.1}s assert={} warmup={:.1}s",
        path.display(),
        duration_secs,
        log_every_secs,
        enabled,
        warmup_secs
    );

    let started = Instant::now();
    let mut last_log = started;
    let mut baseline_ws = 0u64;
    let mut baseline_pf = 0u64;
    let mut sessions = 0u64;
    let mut total_video_frames = 0u64;
    let mut total_audio_frames = 0u64;
    let mut window_video_frames = 0u64;
    let mut checked_geometry = 0u64;
    let mut first_frame_at: Option<Instant> = None;

    while started.elapsed().as_secs_f64() < duration_secs {
        let mut player = MediaPlayer::open(&path, PlayerOptions::default()).unwrap_or_else(|e| {
            panic!("[video-soak] 断言失败：第 {sessions} 次 open 失败：{e}")
        });
        sessions += 1;
        let session_start = Instant::now();
        let mut session_frames = 0u64;
        let mut last_video_at = session_start;

        loop {
            if let Some(frame) = player.poll_video_frame() {
                last_video_at = Instant::now();
                session_frames += 1;
                total_video_frames += 1;
                window_video_frames += 1;
                if first_frame_at.is_none() {
                    first_frame_at = Some(Instant::now());
                }

                // ---- 断言 1：帧几何与格式 ----
                match &frame {
                    VideoFrameSource::Cpu(f) => {
                        let size = f.coded_size();
                        assert!(
                            size.width > 0 && size.height > 0,
                            "[video-soak] 断言失败：帧 coded_size 非法 {}x{}",
                            size.width,
                            size.height
                        );
                        assert!(
                            f.num_planes() > 0,
                            "[video-soak] 断言失败：CPU 帧没有平面"
                        );
                        let layout = f.layout();
                        for i in 0..f.num_planes() {
                            let geom = &layout.planes()[i];
                            let rows = layout.plane_rows(i);
                            let plane = &f.planes()[i];
                            assert!(
                                geom.stride > 0 && rows > 0,
                                "[video-soak] 断言失败：平面 {i} stride={} rows={}",
                                geom.stride,
                                rows
                            );
                            let data = plane
                                .cpu_data()
                                .unwrap_or_else(|| panic!("[video-soak] 断言失败：平面 {i} 无 CPU 数据"));
                            assert!(
                                data.len() >= geom.stride * rows,
                                "[video-soak] 断言失败：平面 {i} 数据 {}B < stride*rows {}B",
                                data.len(),
                                geom.stride * rows
                            );
                        }
                        // 统一转换必须成功，且产物是 NV12、尺寸与 coded_size 一致。
                        let desc = import_video_frame(&frame).unwrap_or_else(|e| {
                            panic!("[video-soak] 断言失败：import_video_frame 失败：{e}")
                        });
                        assert_eq!(
                            desc.size,
                            (size.width, size.height),
                            "[video-soak] 断言失败：FrameDesc 尺寸与 coded_size 不一致"
                        );
                        assert_eq!(
                            desc.format,
                            ExternalTextureFormat::Nv12,
                            "[video-soak] 断言失败：CPU 帧转换结果不是 NV12：{:?}",
                            desc.format
                        );
                        checked_geometry += 1;
                    }
                    VideoFrameSource::Native { .. } => {
                        // 硬解帧没有 CPU 平面，只检查尺寸与转换成功。
                        let desc = import_video_frame(&frame).unwrap_or_else(|e| {
                            panic!("[video-soak] 断言失败：import_video_frame(Native) 失败：{e}")
                        });
                        assert!(
                            desc.size.0 > 0 && desc.size.1 > 0,
                            "[video-soak] 断言失败：Native 帧尺寸非法"
                        );
                    }
                }
            }
            if player.poll_audio_frame().is_some() {
                total_audio_frames += 1;
            }

            // 一帧都没出且超过 stall：解码线程假死。
            if session_frames == 0 && session_start.elapsed().as_secs_f64() >= stall_secs {
                panic!(
                    "[video-soak] 断言失败：会话 {sessions} 在 {stall_secs}s 内没有产出任何帧（解码线程假死）"
                );
            }
            // 出过帧但长时间不再出：视为 EOF，结束本会话去 reopen（即 open/drop 压力）。
            if session_frames > 0 && last_video_at.elapsed().as_secs_f64() >= session_idle_secs {
                break;
            }
            if session_start.elapsed().as_secs_f64() >= duration_secs {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        drop(player);

        // ---- 周期日志 + 断言 2/3/4 ----
        if last_log.elapsed().as_secs_f64() >= log_every_secs {
            let (ws, pf) = memory_kb();
            let elapsed = started.elapsed().as_secs_f64();

            if enabled && elapsed >= warmup_secs {
                if baseline_ws == 0 {
                    baseline_ws = ws;
                    baseline_pf = pf;
                } else if ws > 0 && pf > 0 {
                    let wsg = (ws as f64 / baseline_ws as f64 - 1.0) * 100.0;
                    let pfg = (pf as f64 / baseline_pf as f64 - 1.0) * 100.0;
                    // 基线低于绝对下限时只记录不判定：小基数的百分比没有意义。
                    if baseline_ws >= memory_floor_kb {
                        assert!(
                            wsg <= memory_pct,
                            "[video-soak] 断言失败：工作集 {ws}KB 相对基线 {baseline_ws}KB 增长 {wsg:.1}% > {memory_pct}%"
                        );
                    }
                    if baseline_pf >= memory_floor_kb {
                        assert!(
                            pfg <= memory_pct,
                            "[video-soak] 断言失败：pagefile {pf}KB 相对基线 {baseline_pf}KB 增长 {pfg:.1}% > {memory_pct}%"
                        );
                    }
                }
                assert!(
                    first_frame_at.is_some(),
                    "[video-soak] 断言失败：预热结束仍未出第一帧"
                );
            }

            eprintln!(
                "[video-soak] t={elapsed:.1}s sessions={sessions} vframes={total_video_frames} aframes={total_audio_frames} \
                 window_vframes={window_video_frames} checked_geom={checked_geometry} ws_kb={ws} pf_kb={pf}"
            );
            window_video_frames = 0;
            last_log = Instant::now();
        }
    }

    eprintln!(
        "[video-soak] 完成：sessions={sessions} vframes={total_video_frames} aframes={total_audio_frames} \
         checked_geom={checked_geometry} elapsed={:.1}s",
        started.elapsed().as_secs_f64()
    );
}
