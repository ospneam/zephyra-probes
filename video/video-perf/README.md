# video-perf

视频解码性能基准（headless，跑固定帧数出报告）。与 `video-soak` 分开：
soak 测长稳与几何/内存断言，perf 只测速度并输出可对比的统计报告。

## 运行

把你自己的测试 MP4 放到本地：

```powershell
cd E:\zephyra-probes\video\video-perf
cargo run -- --path E:\videos\test.mp4 --frames 500 --warmup 20
```

`--path` **必填**。默认 H.264 + AAC；其它编码需改 `Cargo.toml` 里
`zephyra-media` 的 feature（如 `video-hevc` / `video-all`）。

## 参数

| 参数 | 默认 | 含义 |
|---|---|---|
| `--path <file>` | 必填 | 测试用 MP4 |
| `--frames N` | 500 | 采集多少帧 |
| `--warmup N` | 20 | 前 N 帧不计入统计 |
| `--session-idle-secs N` | 1.0 | 无新帧多久视为本片结束、reopen 继续采样 |
| `--max-seconds N` | 120 | 总时长上限（防卡死） |
| `--assert-p95-us N` | 无 | poll_import p95 超过此值即 panic（退出码 101） |

## 报告

```
PROFILE video-perf
path=... delivered=80 warmup=10 elapsed_ms=20867
throughput_fps=3.4
poll_import_us n=70 min=121 avg=147.8 p50=142 p95=210 p99=316 max=316
interval_us    n=35 min=124 avg=141.7 p50=131 p95=225 p99=232 max=232
```

- `poll_import_us`：主线程 `poll_video_frame()` + `import_video_frame()` 的耗时
  （消费者侧延迟；解码在 media 的后台线程进行）
- `interval_us`：相邻到达帧的墙钟间隔（帧节奏）；会话 reopen 的间隙不计入
- `throughput_fps`：实测吞吐 = (delivered - warmup) / 总墙钟时间
- 做基线对比时要固定 `--frames` / `--warmup` / 同一个 MP4

## 本机基线（H.264 64×48 测试片，frames=80 warmup=10）

```
poll_import p50≈142µs p95≈210µs   interval p50≈131µs
吞吐受测试片本身 5fps 帧率与频繁 reopen 限制（≈3.4fps），以 poll_import 为准
```
