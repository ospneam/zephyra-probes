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

### 运行期断言（默认开启）

指标一旦偏离预期，**立即 panic**（退出码 101，便于 CI 判定失败），不用等跑完再人工看日志：

| 参数 | 默认 | 判据 |
|---|---|---|
| `--assert-warmup-secs N` | 10 | 预热结束后才建立基线并启用断言 |
| `--assert-memory-pct N` | 10 | 工作集 / pagefile 相对预热基线增长不得超过 N% |
| `--assert-memory-floor-kb N` | 65536 | 基线低于此值（KB）时跳过百分比判定——小基数的抖动没有意义 |
| `--assert-evictions N` | 0 | 图集累计淘汰数上限（稳态应为 0） |
| `--assert-p95-drift-pct N` | 50 | 同一 phase 的 p95 相对该 phase 基线漂移上限 % |
| `--assert-p95-floor-us N` | 2000 | 基线 p95 低于此值（µs）时不判定——样本小、抖动大 |
| `--assert-p95-strikes N` | 3 | p95 需连续 N 次超阈值才 panic（消除单样本尖峰误报） |
| `--assert-quad-drift N` | 0 | 同一 phase 的 quad 数变化上限 |
| `--no-assert` | — | 关闭全部断言（只打日志） |

p95 与 quad 按 phase 分别比较：emoji 场景的 COLR 合成天然比文本场景慢一个
数量级，混在一起比会误报。p95 每个 phase 至少 24 个样本才开始判定；且
**连续**超阈值才失败——debug 构建的帧耗时抖动很大，单个样本的 p95 不能代表
持续回归。日志里带 `phase_p95` 与 `strikes`，便于定位。

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

以下每项都由运行期断言自动检查，违反立即 panic：

- 预热后 `ws_kb` / `pf_kb` 增幅不超过 `--assert-memory-pct`（默认 10%）
- 稳态 `evictions` 不超过 `--assert-evictions`（默认 0）
- 同一 phase 的 p95 不比基线漂移超过 `--assert-p95-drift-pct`（默认 50%）
- 同一 phase 的 quad 数变化不超过 `--assert-quad-drift`（默认 0）

验证断言本身会触发（把阈值压到不合理的极小值）：

```powershell
cargo run -- --assert-warmup-secs 3 --assert-memory-pct 0.01 --duration-secs 12
# thread 'main' panicked: [soak] 断言失败：工作集 ... 增长 1.7% > 0.0%
```

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
