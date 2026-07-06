# 下载任务：开始/完成时间 + 完整文件路径 · 设计文档

## 目标

给 Aria2 下载工具的任务卡片补两类信息：

- **开始时间 / 完成时间 / 用时**（`started_at` / `finished_at`）
- **下载完成后的完整本地文件路径**（`local_path`）

## 数据模型

`download_tasks` 表新增 3 列，均 nullable，通过 `ALTER TABLE ADD COLUMN` 逐一执行（sqlite 无 `IF NOT EXISTS`，用 try-catch 忽略"duplicate column"错误）：

- `started_at TEXT`：首次转 `running` 的时刻（`datetime('now', 'localtime')`）
- `finished_at TEXT`：首次转 `completed` / `failed` / `cancelled` 的时刻
- `local_path TEXT`：aria2 完成时打印的 `Download complete: <path>` 中提取的绝对路径

## 后端改动（`src-tauri/src/lib.rs`）

1. **aria2 参数**：`--console-log-level=warn` → **`notice`**（让 `Download complete:` NOTICE 行进 stdout）
2. `parse_complete_line(line)` 新增：正则 `\[NOTICE\].*Download complete:\s*(.+?)\s*$` 提取 path；命中返回 `Some(path)`
3. `read_aria2_stdout` 里在既有 `parse_progress_line` 分支之外增加 `parse_complete_line` 分支，命中就 emit 一个只带 `local_path` 的 `DownloadEventPayload`（status 仍是 running，进度维持）
4. `DownloadEventPayload` / `DownloadTaskRecord` 各加：
   - `started_at: Option<String>`
   - `finished_at: Option<String>`
   - `local_path: Option<String>`
5. `persist_download_event` 的 UPDATE 加：
   ```sql
   started_at = CASE
     WHEN started_at IS NULL AND ?status = 'running' THEN datetime('now', 'localtime')
     ELSE started_at
   END,
   finished_at = CASE
     WHEN finished_at IS NULL AND ?status IN ('completed', 'failed', 'cancelled') THEN datetime('now', 'localtime')
     ELSE finished_at
   END,
   local_path = COALESCE(?local_path, local_path)
   ```
6. **兜底**：`start_download` 的 `wait()` 分支里，若最终状态是 `completed` 且 `file_name` 由用户显式提供，用 `download_dir / file_name` 合成 `local_path`（覆盖场景：aria2 notice 行被截断错过）
7. `mark_interrupted_tasks`（启动时把 running→failed）：一并补 `finished_at`
8. `load_download_tasks` SELECT 补 3 列

## 前端改动

**类型（`src/types/download.ts`）**

```ts
export type DownloadEventPayload = {
  id: string;
  status: DownloadStatus;
  progress: number;
  speed?: string | null;
  eta?: string | null;
  message?: string | null;
  startedAt?: string | null;
  finishedAt?: string | null;
  localPath?: string | null;
};

export type DownloadTask = DownloadEventPayload & {
  url: string;
  downloadDir: string;
  fileName?: string;
  createdAt: string;
};
```

**merge 逻辑（`src/App.vue`）**：现有 `tasks.value.map(...)` 里 spread `...payload` 已经能自动带上新字段；不用额外改。**新建任务**塞 `tasks.value` 时给 3 个新字段默认 `null`。

**TaskRow（`src/components/ui/TaskRow.vue`）**

进度条下方新增：

```
.time-meta:   开始 14:23:05 · 完成 14:23:47 · 用时 42s
.path-meta:   文件：/Users/.../video.mp4       [复制路径]
```

展示规则：

- queued：`.time-meta` / `.path-meta` 均隐藏
- running：`.time-meta` 只显示"开始 xx:xx:xx"；`.path-meta` 隐藏
- completed：三段时间齐显；有 `localPath` 时显示 `.path-meta`
- failed / cancelled：三段时间齐显；`.path-meta` 隐藏（下载没成，没路径）

**格式化（`src/utils/downloadFormat.ts`，新）**

- `formatClock(sqliteDatetime)`：`'2026-07-07 14:23:05'` → `'14:23:05'`；空/null → `''`
- `formatDurationSeconds(n)`：`< 60` → `Ns`；`< 3600` → `Nm Ns`；否则 `Nh Nm`；负/NaN → `''`
- `computeDurationSeconds(started, finished)`：都合法则算秒差；否则 `null`

## 测试

- 后端：
  - `parse_complete_line("... [NOTICE] Download complete: /tmp/foo.mp4")` → `Some("/tmp/foo.mp4")`
  - `parse_complete_line("[#1 12MiB/50MiB(24%)]")` → `None`
  - `parse_complete_line("[NOTICE] Download complete: /path with spaces/x.mp4")` → `Some("/path with spaces/x.mp4")`
- 前端：`downloadFormat.test.ts` 覆盖 `formatClock` / `formatDurationSeconds` / `computeDurationSeconds` 边界值
- 手动：新建下载 → running 出现"开始"→ 完成后完整三段 + 路径 + 复制路径按钮 → 重启应用后信息仍持久 → 手动 `ALTER TABLE ... DROP COLUMN`（或直接删除 DB）后启动不崩

## 不做

- "在 Finder 中显示"（`open -R`）
- 失败重试
- 使用 `updated_at` 或历史事件的显示
- Aria2 JSON-RPC 改造
