<script lang="ts">
  import type { AppSection } from "../types";
  import Icon from "./Icon.svelte";

  export let active: AppSection;
  export let openTaskCount = 0;
  export let onSelect: (section: AppSection) => void;

  const primary: {
    id: Exclude<AppSection, "settings">;
    label: string;
    icon: "today" | "tasks" | "work" | "money" | "review";
  }[] = [
    { id: "today", label: "Today", icon: "today" },
    { id: "tasks", label: "Tasks", icon: "tasks" },
    { id: "work", label: "Work", icon: "work" },
    { id: "money", label: "Money", icon: "money" },
    { id: "review", label: "Review", icon: "review" },
  ];
</script>

<aside class="sidebar" aria-label="主导航">
  <div class="brand">
    <div class="brand-mark" aria-hidden="true">
      <span>R</span>
    </div>
    <div class="brand-copy">
      <strong>RudeSync</strong>
      <span>个人工作台</span>
    </div>
  </div>

  <nav class="nav-list" aria-label="工作区">
    <span class="nav-label">工作区</span>
    {#each primary as item}
      <button
        class:active={active === item.id}
        class="nav-item"
        type="button"
        aria-current={active === item.id ? "page" : undefined}
        title={item.label}
        on:click={() => onSelect(item.id)}
      >
        <span class="nav-icon"><Icon name={item.icon} size={17} /></span>
        <span class="nav-text">{item.label}</span>
        {#if item.id === "tasks" && openTaskCount > 0}
          <span class="nav-count" aria-label={`${openTaskCount} 个待办`}>
            {openTaskCount}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="sidebar-spacer"></div>

  <div class="sidebar-footer">
    <button
      class:active={active === "settings"}
      class="nav-item"
      type="button"
      aria-current={active === "settings" ? "page" : undefined}
      title="设置"
      on:click={() => onSelect("settings")}
    >
      <span class="nav-icon"><Icon name="settings" size={17} /></span>
      <span class="nav-text">设置</span>
    </button>

    <div class="local-status" title="数据保存在本机">
      <span class="status-dot"></span>
      <span>本地且私密</span>
    </div>
  </div>
</aside>
