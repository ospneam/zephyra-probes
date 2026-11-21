//! 视频播放实验室：D3D12 窗口把视频解码帧作为外部纹理显示出来。
//!
//! 用法：cargo run -- --path <你的测试.mp4>
//! 调试桥可选：$env:ZEPHYRA_DEBUG_PORT=7878

#[cfg(all(windows, feature = "d3d12"))]
mod lab {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use zephyra_core::context::Context;
    use zephyra_core::core::Color;
    use zephyra_core::element::Element;
    use zephyra_core::id::NodeTag;
    use zephyra_core::property::PropertyValue;
    use zephyra_core::props;
    use zephyra_ext::prelude::{ExtContextHandle, ExternalTextureHandle, FrameId};
    use zephyra_fx::View;
    use zephyra_media::{import_video_frame, MediaPlayer, PlayerOptions, VideoFrameSource};

    /// 一个外部纹理的固定 key。每帧 submit 同 key 原地替换内容。
    const VIDEO_KEY: &str = "video-lab-frame";

    /// View 与宿主共享的统计（帧数 / 显示的视频尺寸）。
    #[derive(Default)]
    struct Stats {
        video_frames: AtomicU64,
        audio_frames: AtomicU64,
        width: AtomicU64,
        height: AtomicU64,
    }

    pub(super) struct VideoLab {
        stats: Arc<Stats>,
    }

    impl View for VideoLab {
        fn on_input(&mut self, _state: &zephyra_fx::InputState) {}

        // 视频一直在播：告诉宿主每拍都泵帧（保留式 UI 默认静止不重画）。
        fn wants_frames(&self) -> bool {
            true
        }

        fn view(&self, _ctx: &Context<'_>) -> Element {
            let vf = self.stats.video_frames.load(Ordering::Relaxed);
            let af = self.stats.audio_frames.load(Ordering::Relaxed);
            let w = self.stats.width.load(Ordering::Relaxed);
            let h = self.stats.height.load(Ordering::Relaxed);

            let header = Element::new(NodeTag::Text)
                .with_key("header")
                .with(
                    &props::TEXT,
                    PropertyValue::static_str("视频播放实验室（H.264 / NV12 外部纹理）"),
                )
                .with(&props::TEXT_SIZE, PropertyValue::float(22.0))
                .with(
                    &props::TEXT_COLOR,
                    PropertyValue::color(Color::rgb(0.95, 0.95, 0.98)),
                );

            let stat_text = format!("video={vf} audio={af} size={w}x{h}");
            let status = Element::new(NodeTag::Text)
                .with_key("status")
                .with(&props::TEXT, PropertyValue::Str(stat_text.into()))
                .with(&props::TEXT_SIZE, PropertyValue::float(16.0))
                .with(
                    &props::TEXT_COLOR,
                    PropertyValue::color(Color::rgb(0.7, 0.85, 0.95)),
                );

            let image = Element::new(NodeTag::Image)
                .with_key("video-image")
                .with(&props::IMAGE_TEXTURE, PropertyValue::static_str(VIDEO_KEY))
                .with(&props::WIDTH, PropertyValue::float(960.0))
                .with(&props::HEIGHT, PropertyValue::float(540.0));

            Element::new(NodeTag::Box)
                .with_key("root")
                .with(
                    &props::DIRECTION,
                    PropertyValue::direction(zephyra_core::layout::Direction::Vertical),
                )
                .with(&props::SPACING, PropertyValue::float(10.0))
                .with(
                    &props::BACKGROUND,
                    PropertyValue::color(Color::rgb(0.05, 0.06, 0.08)),
                )
                .with_children(vec![header, status, image])
        }
    }

    type Host = zephyra_winit_d3d12::HostWindow<VideoLab>;

    struct Bootstrap {
        inner: Option<Host>,
        view: Option<VideoLab>,
        stats: Arc<Stats>,
        player: Option<MediaPlayer>,
        /// 扩展上下文：必须比纹理句柄活得久（句柄只持有 Weak 引用）。
        ext: Option<ExtContextHandle>,
        /// 外部纹理句柄：Drop 会 unregister，必须活到窗口结束。
        texture: Option<ExternalTextureHandle>,
    }

    impl Bootstrap {
        /// 取一帧解码结果并提交到当前帧的外部纹理。同 key 反复 submit 即原地替换。
        fn pump_video(&mut self) {
            let Some(ref player) = self.player else { return };
            let frame = player.poll_video_frame();
            let Some(frame) = frame else { return };

            match &frame {
                VideoFrameSource::Cpu(f) => {
                    let size = f.coded_size();
                    self.stats.width.store(size.width as u64, Ordering::Relaxed);
                    self.stats.height.store(size.height as u64, Ordering::Relaxed);
                }
                VideoFrameSource::Native { width, height, .. } => {
                    self.stats.width.store(*width as u64, Ordering::Relaxed);
                    self.stats.height.store(*height as u64, Ordering::Relaxed);
                }
            }
            self.stats.video_frames.fetch_add(1, Ordering::Relaxed);

            let desc = match import_video_frame(&frame) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("[video_lab] import 失败：{e}");
                    return;
                }
            };

            let Some(ref ext) = self.ext else { return };
            // u64::MAX = 永久有效（与 register_texture 兼容入口一致）。
            if let Some(ref handle) = self.texture {
                // 已有 key：原地替换内容（S7）。
                if let Err(e) = handle.update(desc, FrameId(u64::MAX)) {
                    eprintln!("[video_lab] 更新纹理失败：{e}");
                }
            } else {
                // 首次：注册并持有句柄，避免 Drop 时 unregister。
                match ext.submit_frame(VIDEO_KEY, desc, FrameId(u64::MAX)) {
                    Ok(h) => self.texture = Some(h),
                    Err(e) => eprintln!("[video_lab] 注册纹理失败：{e}"),
                }
            }
        }
    }

    impl winit::application::ApplicationHandler for Bootstrap {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if self.inner.is_some() {
                return;
            }
            let view = self.view.take().expect("resumed 只应建窗一次");
            let host = Host::new_lenient_sized(event_loop, view, (1100, 760));
            // 在窗口建好、设备就绪后取一次扩展上下文并长期持有。
            self.ext = Some(host.ext_context());
            self.inner = Some(host);
            eprintln!("[video_lab] 窗口已建立");
        }

        fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            // 先提交解码帧（同 key 原地替换），再让宿主 tick() 自己 frame()/present()。
            // 宿主在 wants_frames()=true 时每拍都画，无需手动调 frame/present。
            self.pump_video();
            if let Some(ref mut host) = self.inner {
                host.about_to_wait(event_loop);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            id: winit::window::WindowId,
            event: winit::event::WindowEvent,
        ) {
            if let Some(ref mut host) = self.inner {
                host.window_event(event_loop, id, event);
            }
        }
    }

    fn path_arg() -> Option<std::path::PathBuf> {
        let args: Vec<String> = std::env::args().collect();
        args.windows(2)
            .find(|w| w[0] == "--path")
            .map(|w| std::path::PathBuf::from(&w[1]))
    }

    pub fn run() {
        let path = path_arg().expect("缺少 --path：cargo run -- --path <测试.mp4>");
        if !path.exists() {
            panic!("视频文件不存在：{}", path.display());
        }

        let player = MediaPlayer::open(&path, PlayerOptions::default())
            .unwrap_or_else(|e| panic!("打开视频失败：{e}"));
        let stats = Arc::new(Stats::default());
        let view = VideoLab {
            stats: stats.clone(),
        };
        let mut bootstrap = Bootstrap {
            inner: None,
            view: Some(view),
            stats,
            player: Some(player),
            ext: None,
            texture: None,
        };

        let event_loop = winit::event_loop::EventLoop::new().expect("EventLoop::new");
        if let Err(e) = event_loop.run_app(&mut bootstrap) {
            eprintln!("[video_lab] 事件循环退出：{e}");
        }
    }
}

fn main() {
    #[cfg(all(windows, feature = "d3d12"))]
    lab::run();

    #[cfg(not(all(windows, feature = "d3d12")))]
    eprintln!("[video_lab] 需要 Windows + d3d12 特性");
}
