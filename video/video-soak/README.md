# video-soak

视频解码长稳（soak）压测应用（headless，无窗口）。反复「打开文件 → 解码全部帧 →
关闭」，在运行期断言四项，违反立即 panic（退出码 101）。

## 运行

把你自己的测试 MP4 放到本地，然后：

```powershell
cd E:\zephyra-probes\video\video-soak
cargo run -- --path E:\videos\test.mp4 --duration-secs 86400
```

`--path` **必填**。默认启用 H.264 软件解码 + AAC + PCM 音频输出；若你的 MP4 是
别的编码，在 `Cargo.toml` 里把 `zephyra-media` 的 feature 换成对应 codec（如
`video-hevc` / `video-av1` / `video-all` 等）。

## 参数

| 参数 | 默认 | 含义 |
|---|---|---|
| `--path <file>` | 必填 | 测试用 MP4 路径 |
| `--duration-secs N` | 60 | 跑多少秒 |
| `--log-every N` | 5 | 每 N 秒打一行指标 |
| `--session-idle-secs N` | 1.5 | 出过帧后多久不再出视为 EOF，结束会话并 reopen |
| `--stall-timeout-secs N` | 5 | 会话内一帧都不出超过此值 ⇒ 假死断言 |
| `--assert-warmup-secs N` | 5 | 内存基线预热时间 |
| `--assert-memory-pct N` | 10 | 工作集 / pagefile 相对基线增长上限 % |
| `--assert-memory-floor-kb N` | 65536 | 基线低于此值（KB）时跳过百分比判定——小基数的抖动没有意义 |
| `--assert-memory-growth-kb N` | 65536 | pagefile 相对历史低水位的绝对增长上限——抓小基数持续泄漏（百分比抓不到） |
| `--assert-growth-grace-secs N` | 600 | 增长断言的观察窗口（秒），给预热/碎片化稳定时间 |
| `--no-audio` | — | 关闭音频解码与输出（诊断用：对比音频路径的内存增长） |
| `--no-assert` | — | 关闭全部断言，只打日志 |

## 运行期断言

1. **帧几何与格式**：每帧 `coded_size>0`、`num_planes>0`、每平面 `stride×rows`
   不超过数据长度；`import_video_frame` 成功且产出 NV12、`FrameDesc` 尺寸与
   `coded_size` 一致。
2. **内存无单调增长**：预热后工作集 / pagefile 相对基线增幅不超过阈值。
3. **持续出帧不卡死**：会话在 `stall-timeout-secs` 内一帧不出 ⇒ panic。
4. **反复 open/drop 无泄漏**：会话循环即 open→解码→drop 压力，内存需稳定。

## 指标

```
[video-soak] t=6.5s sessions=10 vframes=20 aframes=200 window_vframes=10 checked_geom=20 ws_kb=13944 pf_kb=3444
```

- `sessions`：open/drop 轮数；`vframes` / `aframes`：累计视频 / 音频帧
- `window_vframes`：本日志窗口内视频帧（应 >0，否则配合 stall 断言看）
- `checked_geom`：通过几何断言的帧数（应与 vframes 一致）
- `ws_kb` / `pf_kb`：进程工作集 / pagefile（Windows API）

## 本机基线（H.264 64×48 测试片，12 秒）

```
sessions=19  vframes=38  checked_geom=38  ws_kb≈13.9MB  pf_kb≈3.5MB（稳定）
```
