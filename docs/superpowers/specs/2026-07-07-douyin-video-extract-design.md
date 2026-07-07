# 抖音真实视频链接提取 · 设计文档

## 目标

在现有「抖音链接提取」工具基础上，把每条短链解析出的 `https://www.douyin.com/video/<id>` 进一步解析为**可直接下载的 mp4 URL**，并提供一键送到 Aria2 下载工具的联动能力。**无水印优先，无水印拿不到时降级为有水印**并在 UI 标出。

## 范围

- 每条已 resolve 的抖音视频页 URL，后台自动 fetch iesdouyin 分享页并抽 mp4 URL
- UI 每条 entry 新增：视频行（mp4 URL + title + 水印标签）、「复制视频链接」、「下载」按钮
- 「下载」按钮切到 Aria2 工具页并把 mp4 URL append 到 URL textarea（不清空用户已有内容）
- 提取失败：显示明确错误，短链层不受影响

## 非目标

- 图集 / 音乐 / 直播回放 / 短剧多集 / 收藏夹解析
- mp4 URL 可用性主动验证（HEAD 一下）—— 交给 Aria2 下载环节
- 视频 title / desc 之外的元数据（作者、点赞数、封面等）
- 就地下载（复用 Aria2 引擎）
- 反反爬升级链（滑块识别、cookie 池等）—— 命中风控就明确报错让用户手动

## 已知风险

抖音网页端结构频繁更新（历史上每 3~6 个月改一次 `RENDER_DATA` 字段路径）。这个功能有**"半年内可能坏一次要修"** 的天然特征，所以核心解析层要设计为**离线可测**（喂 HTML fixture 走单测），修的时候快速定位。

## 架构

```
[短链] → resolve_douyin_url ──→ [canonical video URL]
                                       │
                                       ▼
                            extract_douyin_video (新)
                                       │
                                       ▼
             { mp4_url, title, has_watermark }
                                       │
             ┌─────────────────────────┼─────────────────────────┐
             ▼                         ▼                         ▼
         复制视频               打开视频页              下载 → useAria2Handoff.push
                                                             + selectTool('aria2')
                                                                    │
                                                                    ▼
                                                      Aria2 分支 drainInto(url)
```

### 文件变更

| 位置 | 变更 |
|---|---|
| `src-tauri/src/douyin.rs`（新） | 独立解析 module：`extract_render_data_from_html`、`find_mp4_url`、`find_title`、`derive_watermark_removed_url` 全部纯函数；`#[cfg(test)]` 单测就地写在 module 内 |
| `src-tauri/src/lib.rs` | `mod douyin;`；新增 `extract_douyin_video(url) -> Result<DouyinVideoInfo, String>` 命令；注册到 `invoke_handler`。原有 `canonicalize_douyin_url` / `douyin_tests` 保持在 `lib.rs`（作用不同：canonicalize 是 302 归一，解析在 douyin.rs） |
| `src/types/douyin.ts`（新） | `DouyinVideoInfo` 类型 |
| `src/composables/useAria2Handoff.ts`（新） | Vue 反应式单例的 pending queue：`push(url)` + `drainInto(target: Ref<string>)` |
| `src/components/douyin/DouyinTool.vue` | Entry 加 `video: { status: 'pending' \| 'ok' \| 'fail', info: DouyinVideoInfo \| null, error: string \| null }`；resolve OK 后自动 kick off video 提取；UI 加视频行 + 两按钮 |
| `src/App.vue` | 抖音工具「下载」→ `handoff.push(url)` + `selectTool('aria2')`；aria2 分支 mount / activate 时 `handoff.drainInto(url)`；用 `watch(selectedToolId)` 触发 |

**新增依赖**：无。`reqwest` / `regex` / `serde_json` 已在；URL decode 用传递依赖 `percent-encoding`（`reqwest → url → percent-encoding`）。

## 解析流程

### 后端 (`extract_douyin_video`)

```
1. 从入参 URL 提取 video_id（regex /video/(\d+)）；不匹配 → 报错
2. reqwest GET https://www.iesdouyin.com/share/video/<id>/
     - UA: DOUYIN_MOBILE_UA (复用 iPhone UA 常量)
     - Accept-Language: zh-CN
     - Accept-Encoding gzip
     - 10s connect / 30s total
     - 最多 10 跳
3. 从 body 用正则抓 <script id="RENDER_DATA" type="application/json">(.+?)</script>
     - 若命中 0：报 "页面结构不识别（可能触发风控）"
4. percent_decode 内容 (URL-encoded UTF-8) → String
5. serde_json::from_str::<serde_json::Value>
     - 失败：报 "响应数据解析失败"
6. 沿路径按顺序尝试抽 mp4 URL（无水印优先）：
     a. $.app.videoDetail.video.playApi        (string)
     b. $.app.videoDetail.video.playAddr[0].src (string)
     c. $.aweme_detail.video.play_addr.url_list[0]  (string)
     d. 兜底：把整个 JSON dump 成字符串，regex 抓第一个 https?://[^"'\s]+\.mp4[^"'\s]*
     e. 全失败：报 "未在页面数据中找到视频 URL"
7. 抽 title:
     a. $.app.videoDetail.desc
     b. $.aweme_detail.desc
     c. 兜底：`douyin_<id>`
8. 水印判定 & 无水印替换：
     - URL 含 "playwm" → has_watermark=true；同时构造 wm_removed = url.replace("playwm", "play")；返回 { url: wm_removed, has_watermark: false }（乐观转换，实际可用性由下载环节验证）
     - URL 含 "play" 且不含 "playwm" → has_watermark=false
     - 其它（兜底 mp4）→ has_watermark=true, 原样返回
9. Return DouyinVideoInfo { mp4_url, title, has_watermark }
```

### 前端

```
resolveOne(entry.short)
  .then(canonical => {
      entry.resolveOk(canonical)
      // 短链层完成后立即 kick off 视频解析
      extractOne(canonical)
        .then(info => entry.videoOk(info))
        .catch(err => entry.videoFail(err))
  })
```

用与短链层同样的 `resolveRun` 计数机制隔离并发批次。

## UI 结构变化

每条 entry 从 2 行变 3 行（保持竖排、`.link-col` 内堆叠）：

```
┌─────────────────────────────────────────────────────────────────┐
│ 1  真实页 URL                                    [复制] [打开]  │
│    短链：https://v.douyin.com/xxx/                              │
│    视频：https://.../play/xxx.mp4  [无水印 · <title>]           │
│                                    [复制视频] [下载]            │
└─────────────────────────────────────────────────────────────────┘
```

**video 三态**：
- pending: 灰字 "解析视频中..."；[复制视频][下载] 按钮 disabled
- ok: 展示 mp4 URL（等宽字体，text-overflow ellipsis over full-width）+ 水印徽章（含/无 water mark 两态）+ title；按钮 enabled
- fail: 红字 "视频解析失败：<error>"；按钮隐藏

**"下载"按钮行为**：
```
1. handoff.push(entry.video.info.mp4_url)
2. selectTool('aria2')
```

**Aria2 页 drain**：
```
watch(selectedToolId, (id) => {
  if (id === 'aria2') handoff.drainInto(url)  // url 是 aria2 的 textarea ref
})
```

第一次进 Aria2 时也 drain（onMounted 时如果 selectedToolId 已经是 aria2）。

`useAria2Handoff` 实现：

```ts
import { ref, type Ref } from 'vue';

const pending = ref<string[]>([]);

export function useAria2Handoff() {
  return {
    push(url: string) {
      pending.value.push(url);
    },
    drainInto(target: Ref<string>) {
      if (!pending.value.length) return;
      const joined = pending.value.join('\n');
      target.value = target.value ? target.value + '\n' + joined : joined;
      pending.value = [];
    },
  };
}
```

## 错误处理

| 场景 | 层级 | 行为 |
|---|---|---|
| HTTP 4xx / 5xx / 超时 / DNS | 后端 | `Err("网络错误：<msg>")` |
| RENDER_DATA 缺失 | 后端 | `Err("页面结构不识别（可能触发风控）")` |
| percent-decode 失败 | 后端 | `Err("响应内容解码失败")` |
| JSON parse 失败 | 后端 | `Err("响应数据解析失败")` |
| 所有 mp4 路径失效 | 后端 | `Err("未在页面数据中找到视频 URL")` |
| title 提取失败 | 后端 | 用 `douyin_<id>` 兜底，不报错 |
| 前端 invoke reject | 前端 | video.status=fail，红字显示 error string |
| 短链 resolve 失败 | 前端 | 短链层照旧显示失败，视频层不再触发 |

## 测试策略

### Rust 单测（`src-tauri/src/douyin.rs`）

- `extract_render_data_from_html(html)`:
  - fixture: `tests/fixtures/douyin_normal.html`（真实抓取的 iesdouyin 分享页）→ 抽出 URL-encoded string
  - fixture: `tests/fixtures/douyin_captcha.html`（滑块页无 RENDER_DATA） → 报 err
- `find_mp4_url(&Value)`:
  - 造 3 份 JSON fixture：路径 a 命中 / 路径 c 命中 / 兜底 regex 命中 / 全失败
- `find_title(&Value)`:
  - 造 fixture：desc 存在 / desc 空
- `derive_watermark_removed_url(url)`:
  - `playwm` → `play` 替换、无 `playwm` 原样返回

fixture HTML 存放：`src-tauri/tests/fixtures/`（复用现有 tests 目录）。或者内联在 `#[cfg(test)]` 用 `include_str!` 引入 `douyin_fixtures/*.html`（更简洁）。

**离线可测的价值**：抖音改结构时，用户可以给你一份新的 iesdouyin HTML，塞成新 fixture，跑测试就能确认修复；不需要联网。

### 前端

- `useAria2Handoff.test.ts`：push → drainInto 追加语义（原为空 / 原非空）；多次 push；drain 后再 push
- 组件手动目视验收

### 手动验收清单

1. 粘一条真实短链 → 短链自动 resolve → 真实页 URL 出来 → 若干秒后视频 mp4 URL + title + 水印徽章出现
2. 「复制视频链接」→ 剪贴板 mp4 URL 正确
3. 「打开」保持原语义（打开真实页 URL）
4. 「下载」→ 切到 Aria2 页 → URL textarea 已 append mp4 URL；若用户已有其它 URL，追加不覆盖
5. Aria2 页手动"开始下载" → 能下到本地 mp4 文件
6. 故意粘一条早已失效的短链 → 视频行显示"视频解析失败：<err>"红字，短链行仍 OK
7. dark / light 主题外观正常
8. 从一开始就在 Aria2 页时点抖音条的"下载" → 切页面后 URL 也追加成功

## 依赖变更

无新的顶层依赖。使用传递依赖 `percent-encoding`（由 `reqwest → url` 引入）；如项目对传递依赖直接使用有偏好，可显式在 `Cargo.toml` 中声明。

## 后续可能

- 抖音改结构时：把新 iesdouyin HTML 加成 fixture、更新 `find_mp4_url` 的路径优先级
- 视频 title 用作 Aria2 下载文件名建议：需要扩展 `useAria2Handoff` 传 fileName（当前范围内不做）
- 图集 / 音频 支持：`RENDER_DATA` 里有对应字段，同样离线可测方式扩展；单独下一份 spec
