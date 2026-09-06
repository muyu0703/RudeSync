const exact = new Map<string, string>([
  ["Today", "今天"],
  ["Tasks", "待办事项"],
  ["Work", "工作"],
  ["Money", "财务"],
  ["Review", "复盘"],
  ["Settings", "设置"],
  ["Open tasks", "待处理"],
  ["Overdue", "已逾期"],
  ["Upcoming", "即将到来"],
  ["This week", "本周"],
  ["Search", "搜索"],
  ["New task", "新建任务"],
  ["Edit task", "编辑任务"],
  ["Task title", "任务标题"],
  ["Notes", "备注"],
  ["optional", "可选"],
  ["Planned date", "计划日期"],
  ["Deadline", "截止日期"],
  ["Priority", "优先级"],
  ["Category", "分类"],
  ["Project", "项目"],
  ["Repeats", "重复"],
  ["Reminder", "提醒"],
  ["Does not repeat", "不重复"],
  ["Daily", "每天"],
  ["Weekdays", "工作日"],
  ["Weekly", "每周"],
  ["Monthly", "每月"],
  ["Custom rule", "自定义规则"],
  ["Repeat every", "每隔"],
  ["Interval", "周期"],
  ["Days", "天"],
  ["Weeks", "周"],
  ["Months", "月"],
  ["Repeat on", "重复日期"],
  ["Subtasks", "子任务"],
  ["Add step", "添加步骤"],
  ["Cancel", "取消"],
  ["Create task", "创建任务"],
  ["Save changes", "保存修改"],
  ["Saving…", "保存中…"],
  ["None", "无"],
  ["Low", "低"],
  ["Medium", "中"],
  ["High", "高"],
  ["Urgent", "紧急"],
  ["Personal / no project", "个人 / 无项目"],
  ["Mon", "周一"],
  ["Tue", "周二"],
  ["Wed", "周三"],
  ["Thu", "周四"],
  ["Fri", "周五"],
  ["Sat", "周六"],
  ["Sun", "周日"],
  ["Clients", "客户"],
  ["Projects", "项目"],
  ["Invoices", "发票"],
  ["Personal loans", "个人借款"],
  ["Earnings", "收入"],
  ["Payments", "收款"],
  ["Create", "创建"],
  ["Add", "添加"],
  ["Delete", "删除"],
  ["Edit", "编辑"],
  ["Close", "关闭"],
  ["Save", "保存"],
  ["Name", "名称"],
  ["Company", "公司"],
  ["Email", "邮箱"],
  ["Status", "状态"],
  ["Amount", "金额"],
  ["Due date", "到期日期"],
  ["Description", "说明"],
  ["Currency", "币种"],
  ["Appearance", "外观"],
  ["General", "常规"],
  ["Notifications", "通知"],
  ["Start with Windows", "开机自动启动"],
  ["Close to tray", "关闭到托盘"],
  ["Backup", "备份"],
  ["Data", "数据"],
  ["Theme", "主题"],
  ["Accent color", "强调色"],
  ["System", "跟随系统"],
  ["Dark", "深色"],
  ["Light", "浅色"],
  ["Local only", "仅本地"],
  ["Open in app", "在主程序中打开"],
  ["No open tasks", "暂无待办"],
  ["Add a note…", "添加备注…"],
  ["Delete?", "确认删除？"],
  ["Refine the plan", "完善任务"],
  ["Capture the work", "记录任务"],
  ["Break the task into small checkable steps.", "把任务拆成可勾选的小步骤。"],
  ["Discard this task? What you've typed will be lost.", "放弃本次编辑吗？已输入的内容会丢失。"],
  ["Give this task a clear title.", "请填写任务标题。"],
  ["The deadline cannot be earlier than the planned date.", "截止日期不能早于计划日期。"],
  ["Choose a valid reminder date and time.", "请选择有效的提醒日期和时间。"],
  ["Custom recurrence must be between 1 and 365.", "自定义重复间隔必须在 1 到 365 之间。"],
  ["Choose at least one weekday for a weekly task.", "每周重复至少选择一天。"],
]);

const placeholders = new Map<string, string>([
  ["Ship the client dashboard", "例如：检查抖店推广数据"],
  ["Context, acceptance criteria, or the next concrete action", "补充说明、执行要求或下一步动作"],
  ["Client work", "例如：店铺运营"],
  ["Describe a step", "填写一个步骤"],
  ["Search RudeSync", "搜索"],
  ["Search tasks", "搜索任务"],
  ["Add a note…", "添加备注…"],
  ["+ new task", "+ 新建待办"],
]);

function translateText(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return value;
  const direct = exact.get(trimmed);
  if (direct) return value.replace(trimmed, direct);

  const patterns: Array<[RegExp, (match: RegExpMatchArray) => string]> = [
    [/^(\d+) open tasks?$/i, (m) => `${m[1]} 个待办`],
    [/^(\d+) overdue$/i, (m) => `${m[1]} 个已逾期`],
    [/^Open tasks \((\d+)\)$/i, (m) => `待办事项（${m[1]}）`],
    [/^Due (.+)$/i, (m) => `到期：${m[1]}`],
    [/^Updated (.+)$/i, (m) => `更新：${m[1]}`],
  ];
  for (const [pattern, build] of patterns) {
    const match = trimmed.match(pattern);
    if (match) return value.replace(trimmed, build(match));
  }
  return value;
}

function translateElement(element: Element): void {
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
    const placeholder = element.getAttribute("placeholder");
    if (placeholder && placeholders.has(placeholder)) {
      element.setAttribute("placeholder", placeholders.get(placeholder)!);
    }
  }
  for (const attribute of ["title", "aria-label"]) {
    const current = element.getAttribute(attribute);
    if (!current) continue;
    const translated = translateText(current);
    if (translated !== current) element.setAttribute(attribute, translated);
  }
}

function translateTree(root: Node): void {
  if (root.nodeType === Node.TEXT_NODE) {
    const text = root.textContent ?? "";
    const translated = translateText(text);
    if (translated !== text) root.textContent = translated;
    return;
  }
  if (root instanceof Element) translateElement(root);
  for (const child of Array.from(root.childNodes)) translateTree(child);
}

export function installChineseUi(): void {
  const run = () => translateTree(document.documentElement);
  run();
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      if (mutation.type === "characterData") {
        translateTree(mutation.target);
        continue;
      }
      for (const node of Array.from(mutation.addedNodes)) translateTree(node);
      if (mutation.target instanceof Element) translateElement(mutation.target);
    }
  });
  observer.observe(document.documentElement, {
    subtree: true,
    childList: true,
    characterData: true,
    attributes: true,
    attributeFilter: ["placeholder", "title", "aria-label"],
  });
}
