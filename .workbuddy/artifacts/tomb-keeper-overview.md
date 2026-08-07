# Tomb Keeper — 家族墓地档案 · Phase 1 交付概览

## 项目目标

以地图标记为核心的家族墓地信息管理应用：记录单墓信息、墓主生平、家族与墓组关系。**离线优先**，面向个人/家族用户。

## 技术选型（已确认）

| 决策项 | 结论 |
|---|---|
| 后端 | Rust（共享核心库 `src-core`） |
| 移动端壳 | Tauri v2（Android） |
| Web 服务 | Axum（可选，Phase 4） |
| 前端 | React 18 + Vite + TypeScript |
| 地图 | MapLibre GL JS |
| 本地存储 | SQLite（rusqlite，WAL + 外键） |

> 否决方案：Flutter（Web 端体验不佳）、Go+Wails+Capacitor（双壳架构割裂）。

## Phase 1 已完成内容

### 1. Rust 核心库（src-core/）

**数据模型**（`models.rs`）：
- `Family` 家族（名称、简介、祖籍）
- `TombGroup` 墓组（属于某家族，如"祖坟区"）
- `Tomb` 单墓地（名称、经纬度、地址、所属家族/墓组）
- `Person` 墓主（称谓、生卒日期、生平、墓志铭，支持合葬多人）
- `Photo` 照片（多态关联 Tomb/Person/Family）

**数据库层**（`db.rs`）：五张表 + 建表迁移 + 全套 CRUD + 按家族聚合查询（含经墓组归属）。**3 个单元测试全部通过**。

### 2. React 前端骨架（web/）

- 侧边栏布局：地图 / 家族 两个入口
- **地图页**：MapLibre GL 渲染 OSM 底图 + 墓地水滴标记 + 弹窗 + 点击跳详情
- **家族列表页** / **家族详情页**（按家族列出墓地）/ **墓地详情页**（位置 + 墓主信息）
- 数据层为内存 Mock（`api/client.ts`），预置"张家"演示数据，Phase 3 换真实后端

## 项目结构

```
tomb-keeper/
├── Cargo.toml              # workspace 根
├── src-core/               # 共享 Rust 核心库
│   ├── src/
│   │   ├── lib.rs          # 模块导出
│   │   ├── models.rs       # 数据模型
│   │   ├── db.rs           # SQLite CRUD + 迁移 + 测试
│   │   └── error.rs        # 统一错误类型
│   └── Cargo.toml
└── web/                    # React 前端
    ├── src/
    │   ├── api/client.ts   # 数据访问层（Mock）
    │   ├── components/     # Layout
    │   ├── pages/          # Map / Families / FamilyDetail / TombDetail
    │   ├── types.ts        # 与 Rust 模型对齐
    │   └── styles.css
    ├── package.json
    ├── tsconfig.json
    └── vite.config.ts
```

## 验证结果

- `cargo build` ✅ 无警告
- `cargo test` ✅ 3/3 通过（家族/墓组 CRUD、墓地/人物 CRUD + 级联删除、照片 CRUD）
- `npm run build` ✅ TypeScript 严格模式零错误
- 开发服务器 http://localhost:5173 ✅ 可访问

## 待办（Phase 2/3）

1. 前端数据层接入真实后端（Tauri invoke / Axum HTTP）
2. Tauri v2 Android 工程集成（GPS 定位、相机拍照）
3. 地图离线瓦片方案（mbtiles 预缓存）
4. 家族树视图、数据导入导出
5. 前端 bundle 代码分割（当前 maplibre-gl 致 978KB）
