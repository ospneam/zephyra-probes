//! 高级文字渲染实验室：一个用 ZephyraUI 框架写的测试应用。
//!
//! 它把刚接进主仓的高级 TextStyle 能力逐条摆在一屏里，便于肉眼验收：
//! 装饰线样式 / skip-ink、阴影、字距与词距、省略号、两端对齐、旋转、
//! 连字符断词、OpenType 特性、可变轴、LCD 子像素、彩色 emoji 与 CJK 回退。
//!
//! 运行：
//!   cd E:\zephyra-gallery
//!   $env:ZEPHYRA_DEBUG_PORT=7878
//!   cargo run -p zephyra-gallery --bin text_lab
//!
//! 默认用 D3D12 后端（与 gallery 一致）。非 Windows 或未开 d3d12 特性时只打印提示。

#[cfg(all(windows, feature = "d3d12"))]
mod lab {
    use zephyra_core::context::Context;
    use zephyra_core::core::Color;
    use zephyra_core::element::Element;
    use zephyra_core::id::NodeTag;
    use zephyra_core::layout::Direction;
    use zephyra_core::property::PropertyValue;
    use zephyra_core::props;
    use zephyra_core::text::{
        LcdOrder, TextAlign, TextDecoration, TextDecorationMode, TextDecorationStyle,
    };
    use zephyra_fx::View;

    const DEMO_FONT: &[u8] = include_bytes!("../fonts/MiSans-Regular.ttf");

    pub(super) fn use_demo_font() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in DEMO_FONT {
            h ^= *b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        let path = std::env::temp_dir().join(format!("zephyra-text-lab-font-{h:016x}.ttf"));
        if !path.exists() {
            if let Err(e) = std::fs::write(&path, DEMO_FONT) {
                eprintln!("[text_lab] 落地字体失败（{e}），中文可能变豆腐块");
                return;
            }
        }
        std::env::set_var(
            zephyra_fx::FONT_OVERRIDE_ENV,
            path.to_string_lossy().as_ref(),
        );
    }

    pub(super) struct TextLab;

    impl View for TextLab {
        fn on_input(&mut self, _state: &zephyra_fx::InputState) {}

        fn view(&self, _ctx: &Context<'_>) -> Element {
            let rows: Vec<Element> = vec![
                row("Skia 风格高级文字实验室", 30.0, |e| {
                    e.with(
                        &props::TEXT_SHADOW,
                        PropertyValue::custom(zephyra_core::text::TextShadow {
                            color: Color::rgba(0.0, 0.0, 0.0, 0.65),
                            offset: zephyra_core::core::Point::new(2.0, 3.0),
                            blur_sigma: 2.0,
                        }),
                    )
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
                    .with(
                        &props::TEXT_DECORATION_MODE,
                        PropertyValue::custom(TextDecorationMode::Gaps),
                    )
                }),
                row("① 字间距 +1.5 / 词间距 +6：CJK 与 Latin 混排 ABC 123", 18.0, |e| {
                    e.with(&props::TEXT_LETTER_SPACING, PropertyValue::float(1.5))
                        .with(&props::TEXT_WORD_SPACING, PropertyValue::float(6.0))
                }),
                row("② 截断：这是一段很长很长很长很长很长很长很长很长很长很长很长的文字，用来验证 max_lines 与省略号", 17.0, |e| {
                    e.with(&props::TEXT_MAX_WIDTH, PropertyValue::float(560.0))
                        .with(&props::TEXT_MAX_LINES, PropertyValue::float(2.0))
                        .with(&props::TEXT_ELLIPSIS, PropertyValue::static_str("…"))
                }),
                row("③ 两端对齐：the quick brown fox jumps over the lazy dog and then runs away", 17.0, |e| {
                    e.with(
                        &props::TEXT_ALIGN,
                        PropertyValue::custom(TextAlign::Justify),
                    )
                    .with(&props::TEXT_MAX_WIDTH, PropertyValue::float(560.0))
                }),
                row("④ 旋转 12° 的文字（绕每个字形中心）", 18.0, |e| {
                    e.with(&props::TEXT_ROTATION, PropertyValue::float(0.21))
                }),
                row("⑤ 连字符断词：internationalization and transcription of complicated documents", 17.0, |e| {
                    e.with(&props::TEXT_HYPHENS, PropertyValue::bool(true))
                        .with(&props::TEXT_MAX_WIDTH, PropertyValue::float(360.0))
                }),
                row("⑥ 表格数字（tnum）：1234567890 / 00.00 / 1,234.56", 18.0, |e| {
                    e.with(
                        &props::FONT_FEATURES,
                        PropertyValue::custom(vec![(u32::from_be_bytes(*b"tnum"), 1u32)]),
                    )
                }),
                row("⑦ 可变字体轴 wght=700（字体不支持时回退伪粗体）", 18.0, |e| {
                    e.with(
                        &props::TEXT_VARIATIONS,
                        PropertyValue::custom(vec![(*b"wght", 700.0f32)]),
                    )
                }),
                row("⑧ LCD 子像素 AA（RGB 排列）：小字清晰度对比", 14.0, |e| {
                    e.with(&props::TEXT_LCD, PropertyValue::custom(LcdOrder::Rgb))
                }),
                row("⑨ 彩色 emoji 与中文混排 😀 🚀 🎉 —— FontSet 自动回退", 24.0, |e| e),
                row("⑩ 行高倍数 1.8：第二行基线由字体真实度量 + 倍数决定", 16.0, |e| {
                    e.with(&props::TEXT_LINE_HEIGHT, PropertyValue::float(1.8))
                }),
            ];

            Element::new(NodeTag::Box)
                .with_key("root")
                .with(
                    &props::DIRECTION,
                    PropertyValue::direction(Direction::Vertical),
                )
                .with(&props::SPACING, PropertyValue::float(12.0))
                .with(
                    &props::BACKGROUND,
                    PropertyValue::color(Color::rgb(0.06, 0.07, 0.09)),
                )
                .with_children(rows)
        }
    }

    fn row(
        text: &'static str,
        size: f32,
        decorate: impl FnOnce(Element) -> Element,
    ) -> Element {
        let base = Element::new(NodeTag::Text)
            .with_key(text)
            .with(&props::TEXT, PropertyValue::static_str(text))
            .with(&props::TEXT_SIZE, PropertyValue::float(size))
            .with(
                &props::TEXT_COLOR,
                PropertyValue::color(Color::rgb(0.95, 0.95, 0.98)),
            )
            .with(&props::TEXT_MAX_WIDTH, PropertyValue::float(900.0));
        decorate(base)
    }

    type Host = zephyra_winit_d3d12::HostWindow<TextLab>;

    struct Bootstrap {
        inner: Option<Host>,
        view: Option<TextLab>,
    }

    impl winit::application::ApplicationHandler for Bootstrap {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if self.inner.is_some() {
                return;
            }
            let view = self.view.take().expect("resumed 只应建窗一次");
            // 调试桥端口由外部环境变量指定，便于用 Zephyra MCP 截图验收。
            let mut app = Host::new_lenient_sized(event_loop, view, (1100, 760));
            app.sync_input();
            let frame = app.frame();
            eprintln!(
                "[text_lab] 首帧 quads={} redrawn={}",
                frame.render.quads, frame.redrawn
            );
            app.present();
            self.inner = Some(app);
        }

        fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if let Some(app) = self.inner.as_mut() {
                app.about_to_wait(event_loop);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            id: winit::window::WindowId,
            event: winit::event::WindowEvent,
        ) {
            if let Some(app) = self.inner.as_mut() {
                app.window_event(event_loop, id, event);
            }
        }
    }

    pub fn run() {
        // 默认用系统字体（FontSet 会按候选表挑 CJK / emoji 字体）。
        // 内嵌 MiSans 只在显式传 --demo-font 时由 main() 注入。
        let event_loop = winit::event_loop::EventLoop::new().expect("EventLoop::new");
        let mut bootstrap = Bootstrap {
            inner: None,
            view: Some(TextLab),
        };
        if let Err(e) = event_loop.run_app(&mut bootstrap) {
            eprintln!("[text_lab] 事件循环退出: {e}");
        }
    }
}

fn main() {
    #[cfg(all(windows, feature = "d3d12"))]
    {
        // 仅在显式要求时才用随应用内嵌的 MiSans；默认走系统字体栈。
        if std::env::args().any(|a| a == "--demo-font") {
            lab::use_demo_font();
        }
        lab::run();
    }

    #[cfg(not(all(windows, feature = "d3d12")))]
    eprintln!("[text_lab] 需要 Windows + d3d12 特性（cargo run --features d3d12）");
}

#[cfg(all(test, windows, feature = "d3d12"))]
mod pixel_regression {
    use super::lab::TextLab;
    use zephyra_core::core::Color;
    use zephyra_fx::cpu_only::CpuHost;

    #[test]
    fn emoji_row_has_real_color() {
        // 系统字体栈自带 Segoe UI Emoji，彩色 emoji 断言不依赖内嵌演示字体。
        let mut host = CpuHost::new(TextLab, (1100, 760));
        host.frame();
        let canvas = host.render_cpu(Color::rgb(0.06, 0.07, 0.09));
        let mut colored = 0usize;
        for y in 397..429 {
            for x in 0..700 {
                let i = (y * 1100 + x) * 4;
                let r = canvas.pixels[i];
                let g = canvas.pixels[i + 1];
                let b = canvas.pixels[i + 2];
                if r.max(g).max(b).saturating_sub(r.min(g).min(b)) > 30 {
                    colored += 1;
                }
            }
        }
        assert!(
            colored > 40,
            "COLR emoji 应产出彩色像素，实得 {colored}；若退化成黑白说明颜色层没接上"
        );
    }
}
