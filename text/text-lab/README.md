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

默认使用 D3D12 后端（Windows）。`ZEPHYRA_DEBUG_PORT` 可选，设了之后可用 Zephyra MCP 截图/查布局。

## 本地联调

`Cargo.toml` 用 git 依赖，仓库可独立克隆。本地开发时，本文件夹的
`.cargo/config.toml`（被根 `.gitignore` 忽略）把 git 依赖覆盖成相邻仓库的
path：`E:/zephyra-vector` 与 `E:/ZephyraUI/...`。改这些路径即可指向你自己的检出目录。

## 字体

`fonts/MiSans-Regular.ttf` 随应用自带，运行时落地到临时目录并通过
`ZEPHYRA_FONT` 环境变量交给 FontSet，保证 CJK 与 emoji 回退的观感稳定。
