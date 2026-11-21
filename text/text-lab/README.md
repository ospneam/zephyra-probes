# text-lab

ZephyraUI 文字渲染的窗口测试应用，对应被测仓库 `zephyra-vector`（文字/排版/图集）。

它把主仓新接的高级 TextStyle 能力逐条摆在一屏里：

1. 标题：文字阴影 + 波浪下划线 + skip-ink
2. 字间距 / 词间距
3. `max_lines` + 省略号
4. 两端对齐
5. 字形旋转
6. 连字符断词
7. OpenType `tnum`
8. 可变字体轴
9. LCD 子像素 AA
10. 彩色 emoji + 中文混排（FontSet 回退）
11. 自定义行高

## 运行

```powershell
cd E:\zephyra-probes\text\text-lab
$env:ZEPHYRA_DEBUG_PORT = 7878
cargo run
```

默认使用 D3D12 后端（Windows），**字体默认走系统字体栈**（FontSet 会按候选表挑 CJK 与 emoji 字体，覆盖 MiSans / Segoe UI Emoji）。`ZEPHYRA_DEBUG_PORT` 可选，设了之后可用 Zephyra MCP 截图/查布局。

若要强制用随应用内嵌的 MiSans（观感固定，脱离系统字体差异），加 `--demo-font` 参数：`cargo run -- --demo-font`。

## 本地联调

`Cargo.toml` 用 git 依赖，仓库可独立克隆。本地开发时，本文件夹的
`.cargo/config.toml`（被根 `.gitignore` 忽略）把 git 依赖覆盖成相邻仓库的
path：`E:/zephyra-vector` 与 `E:/ZephyraUI/...`。改这些路径即可指向你自己的检出目录。

## 回归检查

- 自动化（CPU 像素）：`cargo test -p text-lab`。`pixel_regression::emoji_row_has_real_color` 用
  `CpuHost` 渲染同一份视图，断言 emoji 行出现真实彩色像素——防止 COLR 层退化成黑白。
- GPU 像素（人工/MCP）：用上面的命令启动窗口，设 `ZEPHYRA_DEBUG_PORT=7878`，再用 Zephyra MCP
  的 `screenshot` / `get_layout` 抓帧。判据：emoji 行有彩色像素，其他文本行为灰度；
  `get_layout` 的 11 行文本与预期一致。这一步不进 CI（需要窗口与调试桥），只作为 GPU 侧兜底。

## 字体

`fonts/MiSans-Regular.ttf` 随应用自带，运行时落地到临时目录并通过
`ZEPHYRA_FONT` 环境变量交给 FontSet，保证 CJK 与 emoji 回退的观感稳定。
