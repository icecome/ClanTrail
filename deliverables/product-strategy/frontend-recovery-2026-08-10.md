# 前端恢复与删除原因核查报告（2026-08-10）

> 配套：implementation-status-2026-08-10.md（同类问题首报）。本报告记录「为什么删」的核查结论与恢复操作，结论已取代首报中「前端不可编译」的状态。

## 一、删除原因核查结论

**唯一记录**：提交 `84007f6`（"refactor: 完成项目重命名为族迹 ClanTrail"，作者 Lice，2026-08-09）的 commit message 第 4 点：

> 4. 清理废弃组件与旧类型定义

**没有任何**设计文档、Issue、ADR 或规划文档解释这次删除。DESIGN.md / design-prototype / feature-plans 描述的是 ClanTrail 新设计系统与后续功能，均未提及「删除旧前端」。

### 该删除存在三重自相矛盾，说明它是一次不完整的重命名操作

1. **删了仍被引用的文件**：`84007f6` 整体删除了 `web/src/pages/*`（9 个）与 `web/src/components/*`（5 个），但同一提交的 `App.tsx` 仍在 `import` 这些页面（`ClanListPage`、`GraveDetailPage` 等）。删除后 `App.tsx` 引用悬空。
2. **新增了从未创建的引用**：该提交的 `App.tsx` 还 `import` 了 8 个页面（`MemberDetailPage / BackupPage / PrivacyPage / GpsPage / AboutPage / MemberListPage / MemberDirectPage / GraphPage`），而这些页面在父提交 `b280586` 乃至整个仓库历史中**都不存在** → 该提交从诞生起就无法编译。
3. **父提交本身也是半成品**：`b280586` 的 `App.tsx` 引用 `FamilyListPage / TombDetailPage` 等旧名，但文件已重命名为 `ClanListPage / GraveDetailPage` → `b280586` 提交时即处于「Family→Clan 改名进行中」的中间态。

### 部分合理性 vs 过度删除

- `.workbuddy/memory/2026-08-07.md` 的全会代码审查确认 `FamiliesPage.tsx`、`Layout.tsx`、`types/index.ts` 为死代码（grep 确认无引用），删除这 3 个有依据。
- 但其余 11 个功能文件（含 `ClanListPage`、`GraveDetailPage`、`MapPage`、`SettingsPage`、`Toast` 等核心 UI + 地理定位/导航工具依赖的 `MapNavSheet`）无任何理由被删。

### 最可能的解释

重命名提交**意图按新 DESIGN.md 设计系统重建前端**，并已先行加入了有价值的基建（`utils/location.ts` 的 Tauri 定位权限处理、`utils/navigation.ts` + `MapNavSheet` 的外部地图唤起、`utils/coord.ts` 坐标系转换、2588 行的 `styles.css` 重设计），但**在删除旧页面后、新页面尚未编写/提交时**就提交了，导致留下一个不可编译的树。属「半成品重命名 / 误删」而非有意为之的清理。

## 二、恢复操作

目标：让项目回到可编译、可运行，同时尽量保留重命名提交中有价值的重设计。

| 步骤 | 操作 | 理由 |
|------|------|------|
| 1 | `git checkout b280586 -- web/src` | 拉回最后一个含真实前端代码的快照（10 页面 + 5 组件 + `client.ts` + `types.ts` + `types/index.ts`）。这些是互相自洽的。 |
| 2 | `git checkout 84007f6 -- web/src/App.tsx web/src/styles.css web/src/utils` | 用重命名提交的「设计系统版 App.tsx + 重设计 styles.css + 地理定位/navigation 工具」覆盖回来，保留有价值的工作。 |
| 3 | 编辑 `App.tsx` | 删除 8 个从未创建页面的 import（行 11–18）+ 9 条对应 `<Route>`；保留 8 个真实页面（ClanList / ClanDetail / GraveDetail / Reminders / Map / AddGrave / Settings / MemberForm）+ Toast。 |
| 4 | 修复 `MapNavSheet.tsx` 与 `utils/navigation.ts` | 两处 import/使用了 `Grave` 类型，但 `b280586` 模型实际叫 `Tomb`（无 `Grave`）。统一改回 `Tomb`，对齐真实模型。 |
| 5 | `npm install --no-save @tauri-apps/api@^2 @tauri-apps/plugin-geolocation@^2 @tauri-apps/plugin-opener@^2` | 这 3 个包已在 `package.json` 声明但 `node_modules` 缺失，导致纯 web 类型检查报模块找不到。补齐后验证。 |

> 注意：步骤 1–2 留下的工作树改动**已 staged 但未提交**（因 `git checkout <commit> -- <path>` 会暂存）。是否提交待你确认。

## 三、验证结果

- `npx tsc --noEmit`：**EXIT=0，零类型错误**。
- `npm run build`（vite）：**BUILD_EXIT=0**，53 modules transformed，`dist/index.html` + `dist/assets/index.css`(42.43kB) + `dist/assets/index.js`(205.6kB) 成功生成。

前端已恢复为**可编译、可打包运行**状态。

## 四、当前状态与遗留

### 已恢复可运行（8 个真实页面）
家族列表 / 家族详情 / 墓位详情 / 时序提醒 / 地图 / 新建·编辑墓位 / 设置 / 人物表单 + Toast 组件 + 地理定位/导航工具。

### 仍缺失（规划中但未实现，需按 feature-plans 新建）
- `GraphPage`（关系图谱）→ 对应 `feature-plans/relationship-graph-plan-2026-08-09.md`
- `BackupPage` / `PrivacyPage` / `GpsPage` / `AboutPage`（设置子页）
- `MemberDetailPage` / `MemberListPage` / `MemberDirectPage`（成员详情/列表/直链查看）

这些在重命名提交里被「提前引用」但从未落地。恢复时已从 `App.tsx` 移除了对应路由，避免悬空引用；日后实现时再加回路由即可。

### 次要清理项（无害）
`App.tsx` 的 `isSubFlow` 仍含指向已删子路由的路径字符串（如 `/settings/(backup|privacy|gps|about)`、`/clans/:id/(members|graph)`），已无对应路由，不会触发、不影响编译；实现上述页面时一并更新即可。

## 五、建议
1. **立即提交**恢复结果（建议单独一个 commit，如 `revert: 恢复被误删的前端页面与组件，使项目可编译`）。
2. 后续按 feature-plans 补 `GraphPage` 等 8 个页面，并加回路由。
3. 在重命名这类破坏性提交前，先确保 `npm run build` 通过再提交，避免再次留下不可编译的树。
