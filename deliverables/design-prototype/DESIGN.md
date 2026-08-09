# 族迹 ClanTrail · 设计系统参考文档（DESIGN.md）

> 本文件是族迹 ClanTrail 移动端（国内安卓）设计系统的单一事实来源（Single Source of Truth）。
> 生成自 Ardot 设计画布，经高保真 HTML 原型验证。所有 AI 编程代理（Cursor / Claude Code / CodeBuddy / Google Stitch）均可直接消费本文件还原 UI。
> 设计哲学：**黛青主色 + 纸感留白 + 朱砂克制点缀**；扁平 90% / 玻璃态 8%（仅底栏）/ 新拟态 2%（仅分段控件）；无渐变、无混色、无 emoji。

---

## 1. Visual Theme & Atmosphere（视觉主题与氛围）

族迹是一款宗族谱系与墓位管理应用。设计语言取法国内安卓主流极简风，以沉静的黛青为骨、纸感米白为底，在肃穆与温度之间取得平衡。

- **设计哲学**：克制、秩序、人文。把视觉噪音降到最低，让逝者信息本身成为焦点。
- **视觉基调**：沉静、温润、有序；哀而不伤，敬而不喧。
- **核心视觉特征关键词**：`黛青主色` · `纸感米底` · `扁平等宽描边` · `玻璃态底栏` · `克制朱砂`
- **光影与质感倾向**：
  - 扁平为主（90%）：卡片仅 1px 描边 `#E6E1D8`，无投影或极弱投影。
  - 玻璃态（8%）：**只用于底部 4-Tab 导航栏**（白 0.55 透明 + 背景模糊 + 内外阴影）。
  - 新拟态（2%）：**只用于分段控件/开关**，底色 `#F4F1EA`，双阴影。

---

## 2. Color Palette & Roles（调色板与角色）

所有颜色必须精确到 HEX，并通过 CSS 变量引用。

### Primary Colors（主色 · 黛青）

| 角色 | HEX | CSS 变量 | 使用场景 |
|------|-----|----------|---------|
| 黛青（主色） | `#35525E` | `--dai` | 主按钮、图标、tab 激活态、hero 底、链接、强调数字 |
| 黛青（深） | `#2A414B` | `--dai-deep` | 大阴影色、深色文字变体 |
| 黛青（柔） | `#E8EEF0` | `--dai-soft` | 列表图标底、关系 chip 底、avatar 底 |

### Accent / Interactive（强调色 · 朱砂）

| 角色 | HEX | CSS 变量 | 使用场景 |
|------|-----|----------|---------|
| 朱砂（点缀） | `#B94A3C` | `--zhusha` | **仅**临近忌日徽标（今天 / 3天后）、忌日实心 tag |
| 朱砂（浅底） | `#F6E7E3` | `--zhusha-soft` | 已弃用（统一为朱砂实心，避免浅红噪音） |

> **朱砂克制原则**：朱砂仅在「3 天内的忌日」场景出现，全稿占比 < 5%；节日、远期节点一律用黛青，不溢出到其他场景。

### Neutral / Gray Scale（中性灰阶）

| 角色 | HEX | CSS 变量 | 使用场景 |
|------|-----|----------|---------|
| 强文字 | `#23282B` | `--ink-1` | 标题、主要正文 |
| 中文字 | `#51585C` | `--ink-2` | 标签、次要正文、墓志 |
| 弱文字 | `#8A9095` | `--ink-3` | 副信息、占位符、chevron、提示 |

### Surface & Borders（表面与边框）

| 角色 | HEX | CSS 变量 | 使用场景 |
|------|-----|----------|---------|
| 纸感底 | `#F4F1EA` | `--paper` | 页面背景、隐私条底 |
| 卡片 | `#FFFFFF` | `--card` | 卡片、输入框、icon-btn 底 |
| 白底变体文字 | `#F3F0E9` | `--white-variant` | 黛青底上的反白文字（hero / 按钮） |
| 描边 | `#E6E1D8` | `--line` | 所有卡片/输入/列表的 1px 边框 |

### Semantic Colors（语义色）

- 成功 `#3F8F6B`、警告 `#C08A2E`、错误 `#B94A3C`（复用朱砂）、信息 `#35525E`（复用黛青）。本应用语义色使用极少，仅在 Toast/校验时出现。

### Shadow Colors（阴影色）

- 玻璃内阴影白：`rgba(255, 255, 255, 0.5)`
- 卡片弱投影：`rgba(54, 82, 94, 0.04)` ~ `0.12`
- FAB 投影：`rgba(53, 82, 94, 0.35)`
- 手机外框投影：`rgba(42, 65, 75, 0.22)` / `0.1`

---

## 3. Typography Rules（排版规则）

- **Font Family**：`"Noto Sans SC", -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif`
- **设计哲学**：以字重（400→700）与字号层级建立秩序；标题用 700 撑场，正文用 400/500 保透气；字距克制，不做夸张 tracking。中文排版行高偏松（1.5–1.7）以适配方块字。

### Type Scale（完整层级）

| Token | 用途 | Size | Weight | Line Height | Letter Spacing |
|-------|------|------|--------|-------------|----------------|
| Display | 统计大数字 | 28–30px | 700 | 1 | 0 |
| H1 | 顶栏标题 | 20px | 700 | 1.2 | 0 |
| H2 | 区块标题 | 15px | 600 | 1.4 | 0 |
| H3 | 卡片标题 / 列表主名 | 16px | 600 | 1.4 | 0 |
| Body | 正文 / 墓位说明 | 13.5px | 400/500 | 1.7 | 0 |
| Body-S | 副信息 / 坐标 | 12.5px | 400 | 1.5 | 0 |
| Caption | 标签 / 提示 | 11–12px | 600 | 1.4 | 0 |
| Tab | 底栏文字 | 10px | 500 | 1 | 0 |

---

## 4. Component Stylings（组件样式）

### Buttons（按钮）

```css
/* 主按钮：黛青实心，圆角 14，白字 */
.btn-primary, .btn-primary-block, .btn-full {
  background: var(--dai);
  color: #fff;
  border: none;
  border-radius: 14px;
  padding: 14px 15px;
  font-size: 15px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
}
.btn-primary-block { width: 100%; }
.btn-full { width: 100%; display: flex; align-items: center; justify-content: center; gap: 8px; }

/* 次按钮：描边，圆角 12 */
.locate-btn {
  flex: 1;
  border: 1px solid var(--line);
  border-radius: 12px;
  padding: 12px;
  font-size: 13px;
  background: var(--card);
  color: var(--ink-1);
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
}
.locate-btn.primary { background: var(--dai); color: #fff; border-color: var(--dai); }
```

### FAB（悬浮操作按钮）

```css
.fab {
  position: absolute; right: 16px; bottom: 118px;   /* 离玻璃底栏 24px */
  width: 56px; height: 56px; border-radius: 28px;     /* 正圆 */
  background: var(--dai); color: #fff;
  display: flex; align-items: center; justify-content: center;
  box-shadow: 0 10px 24px rgba(53, 82, 94, 0.35);
  z-index: 30;
}
.fab svg { width: 26px; height: 26px; }
```

### Cards（卡片）

```css
.card {
  background: var(--card);
  border: 1px solid var(--line);
  border-radius: 16px;                /* --radius-card */
  padding: 16px;
}
/* 统计卡 */
.stat-card { flex: 1; background: var(--card); border: 1px solid var(--line); border-radius: 16px; padding: 16px; }
.stat-card .num { font-size: 28px; font-weight: 700; color: var(--dai); line-height: 1; }
.stat-card .lbl { font-size: 13px; color: var(--ink-2); margin-top: 8px; font-weight: 500; }
```

### Inputs（输入框）

```css
.field input, .field textarea {
  width: 100%;
  border: 1px solid var(--line);
  border-radius: 12px;
  padding: 12px 14px;
  font-size: 14px;
  font-family: inherit;
  color: var(--ink-1);
  background: var(--card);
  outline: none;
}
.field input::placeholder { color: var(--ink-3); }
.field input:focus { border-color: var(--dai); }
.field > label { display: block; font-size: 13px; font-weight: 600; color: var(--ink-2); margin-bottom: 7px; }
```

### Navigation（导航 · 玻璃态底栏）

```css
.tabbar {
  position: absolute; left: 20px; right: 20px; bottom: 30px;
  height: 64px; border-radius: 32px;
  background: rgba(255, 255, 255, 0.55);
  -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px);
  border: 1px solid rgba(255, 255, 255, 0.5);
  box-shadow: 0 8px 24px rgba(54, 82, 94, 0.12), inset 0 1px 0 rgba(255, 255, 255, 0.5);
  display: flex; align-items: center; justify-content: space-around;
  padding: 0 8px; z-index: 20;
}
.tab { display: flex; flex-direction: column; align-items: center; gap: 3px; width: 56px; color: var(--ink-3); cursor: pointer; }
.tab svg { width: 23px; height: 23px; }
.tab span { font-size: 10px; font-weight: 500; }
.tab.active { color: var(--dai); }
```

### Badges / Tags（徽标）

```css
.badge { display: inline-flex; align-items: center; font-size: 11px; font-weight: 600; padding: 3px 9px; border-radius: 999px; line-height: 1.4; }
.badge-out-dai { color: var(--dai); border: 1px solid var(--dai); padding: 2px 8px; border-radius: 999px; }   /* 节日/远期：黛青描边 */
.badge-solid-zhu { background: var(--zhusha); color: #fff; padding: 3px 9px; border-radius: 999px; }            /* 临近忌日：朱砂实心 */
.badge-pill-dai { background: var(--dai); color: var(--white-variant); font-size: 11px; font-weight: 600; padding: 3px 10px; border-radius: 999px; }  /* L0 胶囊 */
```

### Modals / Dialogs（对话框 · 通用）

```css
/* 通用：遮罩 + 居中白卡，圆角 16，动画 180ms ease */
.dialog-mask { position: fixed; inset: 0; background: rgba(42, 65, 75, 0.4); display: flex; align-items: center; justify-content: center; }
.dialog { background: var(--card); border-radius: 16px; padding: 20px; width: calc(100% - 48px); max-width: 340px; }
```

---

## 5. Layout Principles（布局原则）

- **Spacing System**：以 **8px** 为基数（4 / 8 / 12 / 16 / 24）。卡片内边距 16，区块间距 18，列表行间距 12。
- **Grid System**：移动端单列，内容安全区 `左右各 20px`；屏框 390×844（iPhone 基准）。
- **Container**：`.content` 绝对定位 `top: 100px`（状态栏44 + 顶栏56）`left/right: 0` `bottom: 0`，`padding: 0 20px`，可滚动。
- **Section Spacing**：`.section-title` 上距 18、下距 10；`.list` 卡片间距 12。
- **留白哲学**：纸感底大面积留白承载肃穆感；内容区从顶栏下 100px 起始，底部预留 140–150px 给 FAB + 玻璃底栏的浮层呼吸。

---

## 6. Depth & Elevation（深度与层级）

### Shadow System

```css
--shadow-xs:  0 1px 3px rgba(54, 82, 94, 0.04);
--shadow-sm:  0 4px 16px rgba(54, 82, 94, 0.04);
--shadow-card: 0 4px 16px rgba(54, 82, 94, 0.04);
--shadow-fab: 0 10px 24px rgba(53, 82, 94, 0.35);
--shadow-phone: 0 24px 60px rgba(42, 65, 75, 0.22), 0 2px 8px rgba(42, 65, 75, 0.1);
--shadow-glass: 0 8px 24px rgba(54, 82, 94, 0.12), inset 0 1px 0 rgba(255, 255, 255, 0.5);
```

### Surface Layers

`background(--paper)` → `surface(--card)` → `elevated(FAB/Dialog)` → `overlay(玻璃底栏/遮罩)`

### Z-index Scale

`content: 1` · `glass tabbar: 20` · `FAB: 30` · `dialog-mask: 100`

### Backdrop Effects（玻璃态）

底栏 `backdrop-filter: blur(18px)` + 白 0.55 透明 + 1px 白 0.5 描边 + 内外阴影。Web 端直接使用 `backdrop-filter` 等价还原 Ardot `BACKGROUND_BLUR(18)`。

---

## 7. Do's and Don'ts（设计规范与禁忌）

**Do's**
1. 主色永远用黛青 `#35525E`；次级信息用三级灰阶区分。
2. 卡片统一 1px `#E6E1D8` 描边 + 16px 圆角，保持扁平一致性。
3. 朱砂只出现在「3 天内忌日」徽标，严格克制。
4. 图标统一 1.6 stroke 线性风格（见图标精灵），圆角容器 11–12px 承托。
5. 列表行采用 `icon(38) + body(flex-1) + chevron(18)` 三段式。
6. 玻璃态仅用于底栏，不滥用到其他浮层。
7. 中英混排时中文用 Noto Sans SC，保持字重层级清晰。

**Don'ts**
1. ❌ 不要使用渐变、混色、霓虹色。
2. ❌ 不要在非忌日场景使用朱砂红。
3. ❌ 不要给卡片加重投影（扁平优先）。
4. ❌ 不要把玻璃态用到卡片、按钮等非导航元素。
5. ❌ 不要使用 emoji 作为图标或装饰。
6. ❌ 不要破坏 8px 间距节奏（避免随意 5/7/13px）。
7. ❌ 不要在非激活 tab 上使用黛青（激活态才上色）。

---

## 8. Responsive Behavior（响应式行为）

| Breakpoint | 宽度 | 策略 |
|------------|------|------|
| mobile | 390–430px | 基准单列，390 屏框 |
| tablet | 600–834px | 居中 390 容器，两侧留白 |
| desktop | ≥1024px | 多屏并排预览（设计稿 stage 模式） |

- **Touch Targets**：可点击元素最小 40×40（icon-btn / tab / row）。
- **折叠策略**：桌面端以「手机外框并排」呈现，不重排为宽屏；功能页面始终单列。
- **Font Scaling**：保持 px 固定值（不随视口缩放），确保设计稿与真机一致。

---

## 9. Agent Prompt Guide（AI 代理提示指南）

### Quick Reference

```
主色 --dai:#35525E | 深 --dai-deep:#2A414B | 柔 --dai-soft:#E8EEF0
点缀 --zhusha:#B94A3C (仅临近忌日) | 纸底 --paper:#F4F1EA | 卡 --card:#FFFFFF
文字 --ink-1:#23282B / --ink-2:#51585C / --ink-3:#8A9095 | 描边 --line:#E6E1D8
字体 Noto Sans SC 400/500/600/700 | 圆角 --radius-card:16px --radius-pill:999px
底栏 玻璃态 blur18 圆角32 | FAB 56圆角28 黛青 离底栏24px
```

### Component Prompts（可直接复制）

1. 「用黛青主色生成一个移动端主按钮，圆角14，白字，hover 加深。」
2. 「生成一个玻璃态底部 4-Tab 导航栏，白0.55透明+blur18，激活态黛青。」
3. 「生成一个忌日提醒卡片：左侧日期块（朱砂/黛青）+ 右侧姓名/墓位 + 朱砂临近徽标。」
4. 「生成一个纸感底表单页：封面虚线区 + 4 个圆角12输入框 + 灰色隐私说明条。」
5. 「生成一个访客账户卡：52圆形avatar + 姓名 + L0胶囊 + chevron。」

### Iteration Guide（迭代建议）

1. 任何新增页面先套 `--paper` 底 + 20px 安全区，不要自创背景色。
2. 新组件必须复用既有 CSS 变量，禁止硬编码色值。
3. 朱砂使用需自检：「是否 3 天内忌日？」否则用黛青。
4. 圆角只有两档：卡片/大块 16px，徽标/药丸 999px，按钮 14px，输入 12px。
5. 图标统一线性 1.6 stroke；新增图标须与精灵风格一致。
6. 底栏与 FAB 永远成对出现（主流程），子流程（添加/编辑）隐藏底栏与 FAB。
7. 间距改动只能落在 8px 倍数（4/8/12/16/24）。
8. 每次改动后用真机视口（390×844）核对，重点检查 FAB 不被底栏遮挡、徽标圆角生效。
9. 墓碑/墓志文字禁用正红，避免文化语义冲突。
10. 中英混排标题层级用字重而非颜色区分，保持肃穆基调。
