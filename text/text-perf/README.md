# text-perf

文本渲染性能基准（headless，跑固定帧数出报告）。与 `text-soak` 分开：
soak 测长稳漂移，perf 只测速度并输出可对比的统计报告。

## 运行

```powershell
cd E:\zephyra-probes\text\text-perf
cargo run -- --frames 600 --warmup 60 --phase-frames 150
```

默认系统字体；用 `CpuHost` 驱动，与 text-soak 相同的 4 个压力场景
（高级特性 / 密集 CJK / emoji COLR / 装饰线+LCD），每帧一个持续变化的旋转
探针强制真重绘。必须 debug 构建（`CpuHost` 是 zephyra-fx 的 debug-only 设施）。

## 参数

| 参数 | 默认 | 含义 |
|---|---|---|
| `--frames N` | 600 | 总帧数 |
| `--warmup N` | 60 | 前 N 帧不计入统计（首帧栅格化全字形） |
| `--phase-frames N` | 150 | 每 N 帧轮换压力场景 |
| `--assert-p95-us N` | 无 | total p95 超过此值即 panic（退出码 101） |

## 报告

```
PROFILE text-perf
frames=200 warmup=30 elapsed_ms=4526
throughput_fps=37.6
total_us  n=170 min=1106 avg=12795.3 p50=1419 p95=3265 p99=414527 max=812769
paint_us  n=170 min=525  avg=8053.2  p50=569  p95=1219 p99=254676 max=693796
expand_us n=170 min=107  avg=506.5   p50=293  p95=1200 p99=1711   max=3464
view_us   n=170 min=25   avg=44.5    p50=34   p95=95   p99=307    max=314
```

- `total_us`：`StageTimes::total_us`（整帧 CPU 生成耗时）
- `paint_us` / `expand_us` / `view_us`：排版+栅格化 / DisplayList→顶点 / 视图构造
- 每个场景的 COLR 合成天然比文本慢一个数量级，所以 p99/max 会被 emoji 场景拉高；
  做基线对比时要固定 `--frames` / `--phase-frames` / `--warmup`。

## 本机基线（debug，frames=200 warmup=30）

```
throughput_fps≈37.6  total p50≈1.4ms p95≈3.3ms  paint p95≈1.2ms
（emoji 场景 p99 达 ~0.4s，系 debug 下 COLR 合成所致，不代表 release 表现）
```
