# video-lab

视频播放实验室：D3D12 窗口把视频解码帧作为外部纹理（NV12）显示出来，叠加
实时帧数 / 尺寸统计。肉眼验收解码 → `FrameDesc` → 外部纹理 → 合成的整条链路。

## 运行

把你自己的测试 MP4 放到本地：

```powershell
cd E:\zephyra-probes\video\video-lab
cargo run -- --path E:\videos\test.mp4
```

`--path` **必填**。窗口默认 1100×760。可选：`$env:ZEPHYRA_DEBUG_PORT=7878`
开启调试桥，用 Zephyra MCP 截图 / 查布局。

## 结构

- `MediaPlayer` 解码，后台线程出帧。
- 每帧 `poll_video_frame()` → `import_video_frame()` 得到 `FrameDesc`。
- 通过 `ExtContextHandle::submit_frame("video-lab-frame", desc, FrameId(u64::MAX))`
  提交为外部纹理；同 key 后续用 `handle.update()` 原地替换（S7）。
- View 里一个 `NodeTag::Image` 元素用 `IMAGE_TEXTURE` 指向该 key 显示画面，
  另两个 Text 元素显示帧数与尺寸。
- `View::wants_frames()` 恒为 true，让保留式 UI 宿主持续泵帧。

## 注意

- 必须 Windows + d3d12 特性（默认开启）。
- 解码 codec 由 `Cargo.toml` 里 `zephyra-media` 的 feature 决定（默认 H.264 + AAC）。
  其它编码需换对应 feature。
- 外部纹理句柄持有 `ExtContextHandle` 的 Weak 引用，所以两者都长期存于 Bootstrap，
  保证纹理在窗口生命周期内一直有效。

## 本机验证

用 media 仓自带的 H.264 测试片（64×48）跑通：窗口显示 6783 种颜色的彩色画面
（测试片的彩条内容），无纹理注册 / 更新错误，帧数持续递增。
