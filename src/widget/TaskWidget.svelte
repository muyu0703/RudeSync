<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { createTaskService } from "../lib/services/backend";
  import { reminderCenter, type ReminderItem } from "../lib/services/reminderCenter";
  import { emitTasksChanged, onTasksChanged, emitOpenTask } from "../lib/services/taskSync";
  import { openTasks } from "./widgetTasks";
  import type { Task } from "../lib/types";

  const service = createTaskService();
  const appWindow = getCurrentWindow();

  let tasks: Task[] = [];
  let reminders: ReminderItem[] = [];
  let quickAddTitle = "";
  let expandedId: string | null = null;
  let menuOpen = false;
  let createMode: "alarm" | "timer" | "task" | null = null;
  let error = "";
  let now = Date.now();

  let alarmTitle = "";
  let alarmAt = "";
  let alarmRepeat: "none" | "daily" | "weekdays" | "custom" = "none";
  let alarmInterval = 3;

  let timerTitle = "";
  let timerHours = 0;
  let timerMinutes = 30;

  $: visibleTasks = openTasks(tasks);
  $: alarms = reminders.filter((item) => item.kind === "alarm");
  $: timers = reminders.filter((item) => item.kind === "timer");

  function normalizeError(value: unknown): string {
    return value instanceof Error ? value.message : String(value ?? "操作失败");
  }

  async function loadAll(): Promise<void> {
    try {
      [tasks, reminders] = await Promise.all([service.listTasks(), reminderCenter.list()]);
      now = Date.now();
      error = "";
    } catch (value) {
      error = normalizeError(value);
    }
  }

  async function mutate(action: () => Promise<unknown>): Promise<void> {
    try {
      await action();
      await loadAll();
      void emitTasksChanged();
      error = "";
    } catch (value) {
      error = normalizeError(value);
    }
  }

  function quickAdd(): void {
    const title = quickAddTitle.trim();
    if (!title) return;
    quickAddTitle = "";
    void mutate(() => service.createTask({ title }));
  }

  function toggleTask(task: Task): void {
    void mutate(() => service.setTaskCompleted(task.id, task.status !== "completed"));
  }

  function openInApp(task: Task): void {
    void emitOpenTask(task.id);
    const tauri = window as unknown as {
      __TAURI_INTERNALS__?: { invoke?: (cmd: string) => Promise<unknown> };
      __TAURI__?: { core?: { invoke?: (cmd: string) => Promise<unknown> } };
    };
    const invoke = tauri.__TAURI_INTERNALS__?.invoke ?? tauri.__TAURI__?.core?.invoke;
    if (invoke) void invoke("focus_main_window");
  }

  function closeWidget(): void {
    void appWindow.hide();
  }

  function startResize(event: PointerEvent, direction: string): void {
    event.preventDefault();
    void appWindow.startResizeDragging(direction as never);
  }

  function chooseMode(mode: "alarm" | "timer" | "task"): void {
    createMode = mode;
    menuOpen = false;
    if (mode === "alarm" && !alarmAt) {
      const date = new Date(Date.now() + 60 * 60 * 1000);
      const offset = date.getTimezoneOffset() * 60_000;
      alarmAt = new Date(date.getTime() - offset).toISOString().slice(0, 16);
    }
  }

  function createAlarm(): void {
    const title = alarmTitle.trim();
    if (!title || !alarmAt) {
      error = "请填写提醒事项和闹钟时间。";
      return;
    }
    const parsed = new Date(alarmAt);
    if (Number.isNaN(parsed.getTime())) {
      error = "闹钟时间无效。";
      return;
    }
    let rule: string | null = null;
    if (alarmRepeat === "daily") rule = "daily";
    if (alarmRepeat === "weekdays") rule = "weekdays";
    if (alarmRepeat === "custom") rule = `FREQ=DAILY;INTERVAL=${Math.max(1, Math.floor(alarmInterval || 1))}`;
    void mutate(async () => {
      await reminderCenter.createAlarm(title, parsed.toISOString(), rule);
      alarmTitle = "";
      alarmRepeat = "none";
      createMode = null;
    });
  }

  function createTimer(): void {
    const title = timerTitle.trim();
    const minutes = Math.max(0, Math.floor(timerHours || 0)) * 60 + Math.max(0, Math.floor(timerMinutes || 0));
    if (!title) {
      error = "请填写倒计时的提醒事项。";
      return;
    }
    if (minutes < 1) {
      error = "倒计时至少需要 1 分钟。";
      return;
    }
    void mutate(async () => {
      await reminderCenter.createTimer(title, minutes);
      timerTitle = "";
      timerHours = 0;
      timerMinutes = 30;
      createMode = null;
    });
  }

  function formatAlarm(at: string | null): string {
    if (!at) return "时间未设置";
    const date = new Date(at);
    if (Number.isNaN(date.getTime())) return at;
    const today = new Date();
    const sameDay =
      date.getFullYear() === today.getFullYear() &&
      date.getMonth() === today.getMonth() &&
      date.getDate() === today.getDate();
    const time = date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
    if (sameDay) return `今天 ${time}`;
    return `${date.getMonth() + 1}月${date.getDate()}日 ${time}`;
  }

  function recurrenceLabel(rule: string | null): string {
    if (!rule) return "仅一次";
    if (rule === "daily") return "每天";
    if (rule === "weekdays") return "工作日";
    const match = rule.match(/^FREQ=DAILY;INTERVAL=(\d+)$/i);
    if (match) return `每 ${match[1]} 天`;
    return "重复";
  }

  function timerMinutesLeft(item: ReminderItem): number {
    if (item.status === "ringing") return 0;
    if (item.status === "paused") return Math.max(0, item.remainingMinutes ?? item.durationMinutes ?? 0);
    if (!item.scheduledAt) return 0;
    const target = new Date(item.scheduledAt).getTime();
    if (!Number.isFinite(target)) return 0;
    return Math.max(0, Math.ceil((target - now) / 60_000));
  }

  function formatDuration(total: number): string {
    const value = Math.max(0, Math.floor(total));
    const hours = Math.floor(value / 60);
    const minutes = value % 60;
    if (hours > 0) return `${hours}小时 ${minutes}分钟`;
    return `${minutes}分钟`;
  }

  function acknowledge(item: ReminderItem): void {
    void mutate(() => reminderCenter.acknowledge(item.id));
  }

  function snooze(item: ReminderItem, minutes: number): void {
    void mutate(() => reminderCenter.snooze(item.id, minutes));
  }

  onMount(() => {
    void loadAll();
    const clock = window.setInterval(() => {
      now = Date.now();
      void reminderCenter.list().then((items) => (reminders = items)).catch(() => {});
    }, 15_000);
    let unsubscribe: (() => void) | null = null;
    void onTasksChanged(() => void loadAll()).then((off) => {
      unsubscribe = off;
    });
    return () => {
      window.clearInterval(clock);
      unsubscribe?.();
    };
  });
</script>

<div class="widget">
  <header data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <span class="brand-dot"></span>
      <span data-tauri-drag-region>提醒与待办</span>
    </div>
    <div class="header-actions">
      <button class="plus" title="快捷新建" aria-label="快捷新建" on:click={() => (menuOpen = !menuOpen)}>＋</button>
      <button class="icon" title="隐藏悬浮窗" aria-label="隐藏悬浮窗" on:click={closeWidget}>×</button>
    </div>
  </header>

  {#if menuOpen}
    <div class="quick-menu">
      <button on:click={() => chooseMode("alarm")}><span>⏰</span>定时闹钟</button>
      <button on:click={() => chooseMode("timer")}><span>⏱</span>倒计时</button>
      <button on:click={() => chooseMode("task")}><span>☑</span>待办事项</button>
    </div>
  {/if}

  {#if createMode === "alarm"}
    <section class="creator alarm-creator">
      <div class="creator-title">新建定时闹钟</div>
      <input bind:value={alarmTitle} maxlength="120" placeholder="提醒事项，例如：准备直播" />
      <input bind:value={alarmAt} type="datetime-local" />
      <div class="creator-row">
        <select bind:value={alarmRepeat}>
          <option value="none">仅一次</option>
          <option value="daily">每天</option>
          <option value="weekdays">工作日</option>
          <option value="custom">每 N 天</option>
        </select>
        {#if alarmRepeat === "custom"}
          <input class="interval" bind:value={alarmInterval} type="number" min="1" max="365" />
          <span>天</span>
        {/if}
      </div>
      <div class="creator-actions">
        <button class="soft" on:click={() => (createMode = null)}>取消</button>
        <button class="primary" on:click={createAlarm}>保存闹钟</button>
      </div>
    </section>
  {:else if createMode === "timer"}
    <section class="creator timer-creator">
      <div class="creator-title">新建倒计时</div>
      <input bind:value={timerTitle} maxlength="120" placeholder="提醒事项，例如：检查库存" />
      <div class="duration-inputs">
        <label><input bind:value={timerHours} type="number" min="0" max="8760" /><span>小时</span></label>
        <label><input bind:value={timerMinutes} type="number" min="0" max="59" /><span>分钟</span></label>
      </div>
      <div class="creator-actions">
        <button class="soft" on:click={() => (createMode = null)}>取消</button>
        <button class="primary" on:click={createTimer}>开始倒计时</button>
      </div>
    </section>
  {:else if createMode === "task"}
    <form class="creator task-creator" on:submit|preventDefault={() => { quickAdd(); createMode = null; }}>
      <div class="creator-title">新建待办事项</div>
      <input bind:value={quickAddTitle} maxlength="120" placeholder="输入待办事项" autofocus />
      <div class="creator-actions">
        <button type="button" class="soft" on:click={() => (createMode = null)}>取消</button>
        <button type="submit" class="primary">添加待办</button>
      </div>
    </form>
  {/if}

  {#if error}
    <div class="error">{error}<button on:click={() => (error = "")}>×</button></div>
  {/if}

  {#if alarms.length > 0}
    <section class="reminder-section">
      <div class="section-label">定时闹钟</div>
      {#each alarms as item (item.id)}
        <article class:ringing={item.status === "ringing"} class="reminder-card alarm-card">
          <div class="reminder-icon">⏰</div>
          <div class="reminder-main">
            <strong>{item.title}</strong>
            {#if item.status === "ringing"}
              <span class="due-text">时间已到</span>
            {:else}
              <span>{formatAlarm(item.scheduledAt)} · {recurrenceLabel(item.recurrenceRule)}</span>
            {/if}
          </div>
          {#if item.status === "ringing"}
            <div class="ring-actions">
              <button class="ack" on:click={() => acknowledge(item)}>我知道了</button>
              <button class="snooze" on:click={() => snooze(item, 10)}>10分钟后</button>
            </div>
          {:else}
            <button class="tiny-delete" title="删除闹钟" on:click={() => mutate(() => reminderCenter.remove(item.id))}>×</button>
          {/if}
        </article>
      {/each}
    </section>
  {/if}

  {#if timers.length > 0}
    <section class="reminder-section">
      <div class="section-label">倒计时</div>
      {#each timers as item (item.id)}
        <article class:ringing={item.status === "ringing"} class="reminder-card timer-card">
          <div class="reminder-icon">⏱</div>
          <div class="reminder-main">
            <strong>{item.title}</strong>
            {#if item.status === "ringing"}
              <span class="due-text">时间已到</span>
            {:else if item.status === "paused"}
              <span>{formatDuration(timerMinutesLeft(item))} · 已暂停</span>
            {:else}
              <span class="countdown">{formatDuration(timerMinutesLeft(item))}</span>
            {/if}
          </div>
          {#if item.status === "ringing"}
            <div class="ring-actions">
              <button class="ack" on:click={() => acknowledge(item)}>我知道了</button>
              <button class="snooze" on:click={() => mutate(() => reminderCenter.reset(item.id))}>重新开始</button>
            </div>
          {:else}
            <div class="timer-actions">
              {#if item.status === "paused"}
                <button on:click={() => mutate(() => reminderCenter.resume(item.id))}>继续</button>
              {:else}
                <button on:click={() => mutate(() => reminderCenter.pause(item.id))}>暂停</button>
              {/if}
              <button on:click={() => mutate(() => reminderCenter.reset(item.id))}>重置</button>
              <button class="danger-text" on:click={() => mutate(() => reminderCenter.remove(item.id))}>删除</button>
            </div>
          {/if}
        </article>
      {/each}
    </section>
  {/if}

  <section class="tasks-section">
    <div class="section-heading">
      <span>待办事项</span>
      <span class="count">{visibleTasks.length}</span>
    </div>

    <ul class="list">
      {#each visibleTasks as task (task.id)}
        <li class="task-row" class:expanded={expandedId === task.id}>
          <div class="task-main">
            <button class="check" role="checkbox" aria-checked={task.status === "completed"} on:click={() => toggleTask(task)}></button>
            <button class="task-title" on:click={() => (expandedId = expandedId === task.id ? null : task.id)}>
              <span class={`priority p-${task.priority}`}></span>
              <span>{task.title}</span>
            </button>
          </div>
          {#if expandedId === task.id}
            <div class="task-detail">
              {#if task.notes}<p>{task.notes}</p>{/if}
              {#if task.subtasks.length}
                <ul>
                  {#each task.subtasks as subtask}
                    <li>{subtask.completed ? "✓" : "○"} {subtask.title}</li>
                  {/each}
                </ul>
              {/if}
              <button on:click={() => openInApp(task)}>在主程序中打开</button>
            </div>
          {/if}
        </li>
      {/each}
      {#if visibleTasks.length === 0}
        <li class="empty">暂无待办事项</li>
      {/if}
    </ul>

    <form class="quick-add" on:submit|preventDefault={quickAdd}>
      <input bind:value={quickAddTitle} maxlength="120" placeholder="+ 新建待办" />
    </form>
  </section>

  <span class="grip e" on:pointerdown={(e) => startResize(e, "East")}></span>
  <span class="grip s" on:pointerdown={(e) => startResize(e, "South")}></span>
  <span class="grip se" on:pointerdown={(e) => startResize(e, "SouthEast")}></span>
</div>

<style>
  .widget {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100vh;
    color: #4d5562;
    background: rgba(251, 249, 247, 0.97);
    border: 1px solid rgba(141, 151, 164, 0.22);
    border-radius: 16px;
    overflow: hidden;
    box-shadow: 0 14px 38px rgba(76, 83, 95, 0.16);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 42px;
    padding: 7px 10px 7px 13px;
    background: linear-gradient(90deg, #eef3f7 0%, #f7f0f2 100%);
    border-bottom: 1px solid rgba(141, 151, 164, 0.13);
    cursor: grab;
  }
  .brand { display: flex; align-items: center; gap: 8px; font-size: 12px; font-weight: 700; color: #596273; }
  .brand-dot { width: 8px; height: 8px; border-radius: 99px; background: #9db8b0; }
  .header-actions { display: flex; align-items: center; gap: 3px; }
  .plus, .icon { border: 0; background: transparent; color: #7b8491; cursor: pointer; border-radius: 8px; }
  .plus { width: 28px; height: 28px; font-size: 20px; line-height: 1; }
  .icon { width: 26px; height: 26px; font-size: 16px; }
  .plus:hover, .icon:hover { background: rgba(255,255,255,.65); color: #4f5967; }

  .quick-menu {
    position: absolute; z-index: 12; top: 38px; right: 10px;
    display: grid; width: 142px; padding: 6px;
    background: #fffdfb; border: 1px solid #e7e1df; border-radius: 12px;
    box-shadow: 0 10px 26px rgba(70, 78, 88, .16);
  }
  .quick-menu button {
    display: flex; align-items: center; gap: 8px; min-height: 34px; padding: 0 9px;
    color: #596273; background: transparent; border: 0; border-radius: 8px; text-align: left; cursor: pointer;
  }
  .quick-menu button:hover { background: #f3f5f6; }

  .creator { display: grid; gap: 8px; padding: 11px 12px; border-bottom: 1px solid #ebe7e4; }
  .alarm-creator { background: #eef5f7; }
  .timer-creator { background: #f4f0f7; }
  .task-creator { background: #f3f6f1; }
  .creator-title { font-size: 11px; font-weight: 700; color: #606a76; }
  .creator input, .creator select {
    width: 100%; height: 32px; padding: 0 9px;
    color: #4f5865; background: rgba(255,255,255,.82);
    border: 1px solid rgba(126, 138, 151, .25); border-radius: 8px; outline: none;
  }
  .creator input:focus, .creator select:focus { border-color: #9eafb9; box-shadow: 0 0 0 3px rgba(167,184,193,.18); }
  .creator-row { display: flex; align-items: center; gap: 6px; }
  .creator-row select { flex: 1; }
  .creator-row .interval { width: 58px; }
  .duration-inputs { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .duration-inputs label { display: grid; grid-template-columns: 1fr auto; align-items: center; gap: 6px; font-size: 11px; }
  .creator-actions { display: flex; justify-content: flex-end; gap: 6px; }
  .creator-actions button { min-height: 29px; padding: 0 10px; border-radius: 8px; border: 0; cursor: pointer; }
  .soft { color: #6d7681; background: rgba(255,255,255,.55); }
  .primary { color: #44504e; background: #c9ded7; font-weight: 650; }

  .error { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 7px 10px; color: #965f63; background: #f8e6e7; font-size: 10px; }
  .error button { border: 0; background: transparent; color: inherit; cursor: pointer; }

  .reminder-section { padding: 8px 8px 0; }
  .section-label { padding: 0 4px 5px; color: #8b929c; font-size: 10px; font-weight: 700; letter-spacing: .04em; }
  .reminder-card {
    display: grid; grid-template-columns: 26px minmax(0,1fr) auto; align-items: center; gap: 8px;
    margin-bottom: 6px; padding: 9px 9px; border-radius: 11px; border: 1px solid transparent;
  }
  .alarm-card { background: #edf5f8; border-color: #d9e8ed; }
  .timer-card { background: #f2eef7; border-color: #e4dcef; }
  .reminder-card.ringing {
    background: #f9dfdc;
    border-color: #e8aaa4;
    box-shadow: 0 0 0 2px rgba(226, 137, 129, .12);
    animation: softPulse 2s ease-in-out infinite;
  }
  @keyframes softPulse { 50% { box-shadow: 0 0 0 5px rgba(226,137,129,.10); } }
  .reminder-icon { font-size: 16px; text-align: center; }
  .reminder-main { min-width: 0; display: grid; gap: 2px; }
  .reminder-main strong { overflow: hidden; color: #4f5863; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .reminder-main span { color: #838b95; font-size: 10px; }
  .reminder-main .countdown { color: #756f84; font-size: 11px; font-weight: 700; }
  .reminder-main .due-text { color: #ad5c5f; font-weight: 750; }
  .tiny-delete { width: 24px; height: 24px; border: 0; background: transparent; color: #a0a6ad; border-radius: 7px; cursor: pointer; }
  .ring-actions { display: flex; flex-direction: column; gap: 4px; }
  .ring-actions button, .timer-actions button {
    min-height: 25px; padding: 0 7px; border: 0; border-radius: 7px; font-size: 9px; cursor: pointer;
  }
  .ack { color: #75484b; background: #efbeb9; font-weight: 750; }
  .snooze { color: #725e60; background: rgba(255,255,255,.55); }
  .timer-actions { display: flex; gap: 3px; flex-wrap: wrap; justify-content: flex-end; max-width: 90px; }
  .timer-actions button { color: #6c6678; background: rgba(255,255,255,.62); }
  .timer-actions .danger-text { color: #a36b70; }

  .tasks-section { min-height: 0; display: flex; flex: 1; flex-direction: column; padding-top: 6px; }
  .section-heading { display: flex; align-items: center; gap: 7px; padding: 5px 12px; color: #6c7480; font-size: 11px; font-weight: 750; }
  .count { display: grid; min-width: 18px; height: 18px; padding: 0 5px; place-items: center; color: #748079; background: #e7eee9; border-radius: 99px; font-size: 9px; }
  .list { flex: 1; margin: 0; padding: 0 6px 5px; list-style: none; overflow-y: auto; }
  .task-row { border-radius: 9px; }
  .task-row:hover { background: #f2f3f2; }
  .task-main { display: flex; align-items: center; gap: 7px; min-height: 34px; padding: 4px 7px; }
  .check { width: 15px; height: 15px; flex: none; background: #fff; border: 1.5px solid #b8c1bf; border-radius: 5px; cursor: pointer; }
  .check[aria-checked="true"] { background: #b7d0c7; border-color: #9cb9af; }
  .task-title { display: flex; align-items: center; gap: 7px; flex: 1; min-width: 0; padding: 0; color: #59616d; background: transparent; border: 0; text-align: left; cursor: pointer; }
  .task-title span:last-child { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
  .priority { width: 6px; height: 6px; flex: none; border-radius: 99px; background: #c0c6c4; }
  .p-urgent { background: #dfa39f; } .p-high { background: #e7bf8f; } .p-medium { background: #a9c7bc; } .p-low { background: #a9bed1; }
  .task-detail { margin: 0 7px 6px 29px; padding: 7px 8px; color: #7a828c; background: #f7f6f4; border-radius: 8px; font-size: 10px; }
  .task-detail p { margin: 0 0 5px; white-space: pre-wrap; }
  .task-detail ul { margin: 0 0 5px; padding-left: 14px; }
  .task-detail button { padding: 3px 7px; color: #66736e; background: #e5ece8; border: 0; border-radius: 6px; font-size: 9px; cursor: pointer; }
  .empty { padding: 18px 10px; color: #a1a6ad; font-size: 10px; text-align: center; }
  .quick-add { padding: 7px; border-top: 1px solid #ece8e5; }
  .quick-add input { width: 100%; height: 31px; padding: 0 10px; color: #59616d; background: #f7f6f4; border: 1px solid #e6e1de; border-radius: 9px; outline: none; }
  .quick-add input:focus { border-color: #bdc9c5; box-shadow: 0 0 0 3px rgba(178,197,190,.14); }

  .grip { position: absolute; z-index: 20; }
  .grip.e { top: 0; right: 0; width: 6px; height: 100%; cursor: ew-resize; }
  .grip.s { left: 0; bottom: 0; width: 100%; height: 6px; cursor: ns-resize; }
  .grip.se { right: 0; bottom: 0; width: 12px; height: 12px; cursor: nwse-resize; }
</style>
