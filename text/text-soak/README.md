# text-soak

文字渲染长稳（soak）压测应用，验证 Zephyra 文字栈在长时间运行下是否出现
内存泄漏、图集抖动或帧耗时漂移。

## 运行

```powershell
cd E:\zephyra-probes\text\text-soak
cargo run -- --phase-frames 300 --log-every 300 --duration-secs 86400
```

> **必须用 debug 构建**：驱动用的 `CpuHost` 是 `zephyra-fx` 的 debug-only dev
> 设施（`#[cfg(debug_assertions)]`），release 下不存在。长稳验证关心的是
> **内存/图集/帧耗时的相对漂移**，debug 下依然有效；绝对耗时偏悲观属正常。

默认使用**系统字体栈**。可选参数：

- `--phase-frames N`：每 N 帧轮换一个压力场景（默认 300）
- `--log-every N`：每 N 帧输出一行指标（默认 300）
- `--fps N`：目标节流帧率（默认 60）
- `--warmup-frames N`：前 N 帧不进 p95（默认 20）
- `--duration-secs N`：跑满 N 秒自动退出（默认 0 = 一直跑）

## 压力场景

按 `frame / phase-frames % 4` 轮换：

0. 高级特性全开（字距/词距/阴影/表格数字/可变轴/LCD 开关）
1. 密集 CJK + 多字号 + 断行
2. 大量 emoji（COLR paint graph 合成压力）
3. 装饰线 + 阴影 + 旋转 + LCD 开关

## 指标

每 `log-every` 帧向 stderr 输出一行：

```
[soak] t=89.3s frame=5640 fps=65.6 frame_us=176294 paint_us=59297 expand_us=306
       p95_us=335460 quads=929 glyphs=81 evictions=0 atlas_used_px=460
       ws_kb=48484 pf_kb=45980
```

- `frame_us`：`StageTimes::total_us`（整帧 CPU 耗时）
- `paint_us` / `expand_us`：文本排版+栅格化 / DisplayList→顶点展开
- `p95_us`：最近 512 个重绘帧的 p95（跳过预热帧）
- `glyphs` / `evictions` / `atlas_used_px`：图集统计；稳态 `evictions` 应为 0
- `ws_kb` / `pf_kb`：进程工作集 / pagefile 占用（Windows API）

每个 phase 内加了一个持续变化的旋转探针，保证每帧都真正重绘（`paint_us` 不为 0），
同时不引入新字形，避免图集被无界撑大。

## 验收标准

- 预热后 24 小时内 `ws_kb` / `pf_kb` 增幅 < 10%
- 稳态 `evictions` 保持 0，不持续增长
- p95_us 不持续上升
- 无崩溃、无错误日志、quad 数稳定

## 本机基线（debug，90 秒 / 5700 帧）

```
ws_kb    ~48,000 → ~48,700（锯齿波动，无单调增长）
pf_kb    ~45,000 → ~46,000（稳定）
evictions 0（全程）
glyphs / atlas_used_px 81 / 460（恒定）
p95_us   ~330,000（emoji 场景 COLR 合成主导，无漂移；非 emoji 场景 ~14,000）
```

> 本机没有 CBDT 字体；emoji 场景走的是 Segoe UI Emoji 的 COLR v1 路径。
> debug 构建下 emoji 的 COLR 合成每帧约 50–110 ms，绝对耗时不代表 release 表现。
