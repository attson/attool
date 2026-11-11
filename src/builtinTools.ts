import type { Tool } from './types/tool';

export const builtinTools: Tool[] = [
  { id: 'app-management', name: '应用管理', description: '安装、启停和卸载独立应用', status: 'ready', icon: 'apps', group: 'platform', source: 'platform' },
  { id: 'aria2', name: 'Aria2 下载', description: 'HTTP / HTTPS / FTP / BT 多连接下载', status: 'ready', icon: 'download', group: 'download' },
  { id: 'template', name: '主图模板', description: 'PSD 导入、字段替换、批量生成主图', status: 'ready', icon: 'layout', group: 'edit' },
  { id: 'clipboard', name: '剪贴板工具', description: 'Paste 风格剪贴板历史与快捷恢复', status: 'ready', icon: 'clipboard', group: 'edit' },
  { id: 'json', name: 'JSON 工具', description: '格式化 / 查询 / 对比 / 转换', status: 'ready', icon: 'code', group: 'edit' },
  { id: 'video-link', name: '视频链接抽取', description: '抖音 / 小红书 / B站 / YouTube（后三者建设中）', status: 'ready', icon: 'video', group: 'download' },
  { id: 'image', name: '图片工具', description: '截图 / 录屏 / 标注 / 压缩 / 转换 / OCR', status: 'ready', icon: 'image', group: 'edit' },
  { id: 'network', name: '网络工具', description: 'URL 分析 / Ping / 端口检查 / DNS', status: 'ready', icon: 'wifi', group: 'network' },
  { id: 'generator', name: '生成器', description: '密码 / UUID / QR / Lorem / 假数据 / 骰子', status: 'ready', icon: 'dice', group: 'utility' },
  { id: 'time', name: '时间工具', description: '时间戳 / 时区 / Cron / Duration', status: 'ready', icon: 'clock', group: 'utility' },
  { id: 'http', name: 'HTTP 请求', description: 'GET/POST/... + headers / body / 响应', status: 'ready', icon: 'send', group: 'network' },
  { id: 'ai-chat', name: 'AI 对话', description: '多提供商多模型 AI 助手（OpenAI 兼容 / Claude / Ollama）', status: 'ready', icon: 'chat', group: 'utility' }
];
