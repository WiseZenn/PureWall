# PureWall - 桌面壁纸美学管家 开发计划

## Context

从零开始构建 PureWall —— 一个 Windows 桌面壁纸管理应用。解决 Windows 原生壁纸轮播无法快速收藏/删除壁纸的痛点，提供 macOS 级极简美学的壁纸管理体验。

**当前环境**：Node.js v24 + npm + Git + .NET SDK 已安装，Rust 未安装。

---

## 技术栈选型：Tauri 2 + Vue 3 + TypeScript

**选择理由**：
- Rust 后端天然适合调用 Windows API（`SystemParametersInfoW`、Shell 扩展、进程检测）
- Web 前端实现 macOS 级毛玻璃/动画/瀑布流 UI 无技术障碍
- 安装包 ~5-10MB，内存占用 ~50-100MB（远优于 Electron 的 200MB+）
- Tauri 2 是当前稳定版本，插件生态成熟

**前置依赖安装**：
1. 安装 Rust 工具链（`rustup`）
2. 安装 WebView2 Runtime（Windows 11 已预装）
3. 安装 Visual Studio Build Tools（C++ 构建工具，Rust linker 需要）

---

## Phase 1: MVP（核心轮播 + 基础交互）

### Step 1: 项目脚手架搭建
- `npm create tauri-app@latest` 初始化 Tauri 2 + Vue 3 + TypeScript 项目
- 配置项目结构：
  ```
  PureWall/
  ├── src/                    # Vue 前端
  │   ├── components/         # UI 组件
  │   ├── stores/             # Pinia 状态管理
  │   ├── lib/                # 工具函数
  │   └── App.vue
  ├── src-tauri/              # Rust 后端
  │   ├── src/
  │   │   ├── main.rs         # 入口 + 插件注册
  │   │   ├── wallpaper.rs    # 壁纸切换核心
  │   │   ├── tray.rs         # 系统托盘
  │   │   ├── db.rs           # SQLite 数据库
  │   │   └── scanner.rs      # 文件夹扫描
  │   ├── Cargo.toml
  │   └── tauri.conf.json
  └── package.json
  ```

### Step 2: Rust 后端核心 — 壁纸引擎（`wallpaper.rs`）
- 使用 `windows` crate 调用 `SystemParametersInfoW(SPI_SETDESKWALLPAPER, ...)`
- 封装 `set_wallpaper(path: &str)` 函数
- 支持 BMP 转换（Windows API 要求）或注册表路径方式
- 实现多显示器支持（`SPIF_SENDCHANGE` + 注册表路径控制）

**关键依赖**：`windows` crate, `image` crate（格式转换）

### Step 3: 文件夹扫描器（`scanner.rs`）
- 递归扫描指定文件夹，收集图片文件（jpg/png/bmp/webp）
- 使用 `notify` crate 监听文件系统变化，实时同步壁纸库
- 计算文件哈希（用于去重），收集文件元数据

### Step 4: SQLite 数据库层（`db.rs`）
- 使用 `rusqlite` crate
- 核心表结构：
  ```sql
  CREATE TABLE wallpapers (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    hash TEXT,
    source TEXT NOT NULL,         -- 'mounted' | 'builtin'
    rating INTEGER DEFAULT 0,     -- 1=liked, -1=disliked, 0=neutral
    play_count INTEGER DEFAULT 0,
    last_played TEXT,
    created_at TEXT DEFAULT (datetime('now'))
  );
  ```
- 基础 CRUD 操作

### Step 5: 播放引擎 — 随机轮播逻辑
- 定时器触发壁纸切换（可配置间隔，默认 10 分钟）
- 加权随机算法：liked 壁纸权重 > 普通 > disliked（已排除）
- Tauri Command 暴露给前端：`next_wallpaper()`, `like_wallpaper()`, `dislike_wallpaper()`, `delete_wallpaper()`

### Step 6: 系统托盘（`tray.rs`）
- 使用 Tauri 2 内置 tray plugin
- 托盘菜单：下一张 / 喜欢 ❤️ / 不喜欢 👎 / 暂停轮播 / 打开主界面 / 退出
- 托盘图标显示状态（轮播中 vs 已暂停）

### Step 7: 前端基础 UI
- 主窗口：壁纸画廊（网格/瀑布流布局）
- 使用 Tailwind CSS + CSS backdrop-filter 实现毛玻璃效果
- 壁纸卡片：缩略图 + 悬浮操作按钮（喜欢/设为壁纸/删除）
- Pinia store 管理壁纸列表、筛选状态

---

## Phase 2: 交互强化（后续迭代）

- 桌面右键菜单注入（注册表修改 `HKEY_CLASSES_ROOT\Directory\Background\shell`）
- 桌面悬浮挂件（透明置顶小窗口，提供 ❤️/🗑️/⏭️ 按键）
- 播放统计面板（播放次数、最近播放时间）
- 开机自启 + 锁屏/解锁自动恢复轮播

## Phase 3: 画廊与高阶管理（后续迭代）

- macOS 风格侧边栏 + 瀑布流画廊
- 标签系统（自定义 Tags）
- 批量操作（多选后打标签/删除/移动）
- 黑名单管理
- 按喜欢/播放次数/最近播放排序

## Phase 4: 打磨（后续迭代）

- 多显示器独立轮播 / 拼接模式
- 游戏/专注模式（检测全屏应用自动暂停）
- 年度壁纸统计仪表盘

---

## 关键文件说明

| 文件                         | 用途                            |
| ---------------------------- | ------------------------------- |
| `src-tauri/src/wallpaper.rs` | 壁纸切换核心，调用 Windows API  |
| `src-tauri/src/scanner.rs`   | 文件夹扫描 + 文件监听           |
| `src-tauri/src/db.rs`        | SQLite 数据库操作               |
| `src-tauri/src/tray.rs`      | 系统托盘菜单                    |
| `src-tauri/src/main.rs`      | Tauri 入口，注册插件和 commands |
| `src/App.vue`                | 前端主界面                      |
| `src/stores/wallpapers.ts`   | Pinia 状态管理                  |
| `src-tauri/tauri.conf.json`  | Tauri 配置（窗口、权限、打包）  |

## 核心 Rust 依赖

| Crate                  | 用途                                   |
| ---------------------- | -------------------------------------- |
| `windows`              | 调用 Windows API（壁纸设置、进程检测） |
| `rusqlite`             | SQLite 数据库                          |
| `notify`               | 文件系统监听                           |
| `image`                | 图片格式转换（BMP for Windows API）    |
| `serde` / `serde_json` | 序列化（前后端通信）                   |
| `tauri`                | 应用框架 + 托盘 + 窗口管理             |

## 验证方式

1. **编译验证**：`cargo build` 确认 Rust 后端编译通过
2. **功能验证**：`npm run tauri dev` 启动开发模式
   - 选择一个图片文件夹 → 托盘出现图标
   - 点击"下一张" → 系统壁纸切换
   - 点击"喜欢" → 数据库记录 rating=1
   - 等待定时器触发 → 自动轮播
3. **UI 验证**：主界面显示壁纸缩略图网格，点击操作按钮生效