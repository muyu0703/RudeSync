<script lang="ts">
  export let name:
    | "today"
    | "tasks"
    | "work"
    | "money"
    | "review"
    | "settings"
    | "plus"
    | "search"
    | "bell"
    | "chevron-right"
    | "calendar"
    | "check"
    | "clock"
    | "invoice"
    | "loan"
    | "arrow-up-right"
    | "more"
    | "link"
    | "spark"
    | "tray"
    | "database"
    | "palette"
    | "shield"
    | "keyboard"
    | "circle"
    | "x"
    | "briefcase";
  export let size = 18;
  export let strokeWidth = 1.8;
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "today"}
    <rect x="3" y="4.5" width="18" height="16" rx="3" />
    <path d="M8 2.5v4M16 2.5v4M3 9h18" />
    <path d="m8.2 14 2.1 2.1 4.7-4.7" />
  {:else if name === "tasks"}
    <path d="M9 6h11M9 12h11M9 18h11" />
    <path d="m3.5 6 1 1 2-2M3.5 12l1 1 2-2M3.5 18l1 1 2-2" />
  {:else if name === "work" || name === "briefcase"}
    <rect x="3" y="6.5" width="18" height="13" rx="2.5" />
    <path d="M9 6.5V4.8c0-.7.6-1.3 1.3-1.3h3.4c.7 0 1.3.6 1.3 1.3v1.7M3 11.5c5.9 3 12.1 3 18 0M10 13h4" />
  {:else if name === "money"}
    <rect x="3" y="5" width="18" height="14" rx="3" />
    <path d="M7 9h.01M17 15h.01" />
    <circle cx="12" cy="12" r="2.5" />
  {:else if name === "review"}
    <path d="M4 19V9M10 19V5M16 19v-7M22 19V3" />
    <path d="M2 19h20" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" />
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "search"}
    <circle cx="10.8" cy="10.8" r="6.8" />
    <path d="m16 16 4.2 4.2" />
  {:else if name === "bell"}
    <path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 8.5h18C21 15 18 15 18 8ZM10 20h4" />
  {:else if name === "chevron-right"}
    <path d="m9 5 7 7-7 7" />
  {:else if name === "calendar"}
    <rect x="3" y="5" width="18" height="16" rx="2.5" />
    <path d="M8 3v4M16 3v4M3 10h18" />
  {:else if name === "check"}
    <path d="m5 12.5 4.2 4.2L19.5 6.5" />
  {:else if name === "clock"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v5l3.5 2" />
  {:else if name === "invoice"}
    <path d="M6 2.8h9l4 4V21H6z" />
    <path d="M15 2.8V7h4M9 11h6M9 15h6" />
  {:else if name === "loan"}
    <path d="M4 8h16v11H4zM2.5 8 12 3l9.5 5M8 11v5M12 11v5M16 11v5M2.5 19h19" />
  {:else if name === "arrow-up-right"}
    <path d="M7 17 17 7M8 7h9v9" />
  {:else if name === "more"}
    <circle cx="5" cy="12" r=".8" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r=".8" fill="currentColor" stroke="none" />
    <circle cx="19" cy="12" r=".8" fill="currentColor" stroke="none" />
  {:else if name === "link"}
    <path d="M10 13.8a4 4 0 0 0 5.7.1l2.2-2.2a4 4 0 0 0-5.7-5.7L11 7.2M14 10.2a4 4 0 0 0-5.7-.1l-2.2 2.2A4 4 0 0 0 11.8 18l1.2-1.2" />
  {:else if name === "spark"}
    <path d="m12 2 1.4 5.1L18 10l-4.6 2.9L12 18l-1.4-5.1L6 10l4.6-2.9L12 2Z" />
    <path d="m19 15 .6 2.3L22 18.5l-2.4 1.2L19 22l-.6-2.3-2.4-1.2 2.4-1.2L19 15Z" />
  {:else if name === "tray"}
    <path d="M4 4h16v16H4zM4 15h4l1.5 2h5l1.5-2h4" />
  {:else if name === "database"}
    <ellipse cx="12" cy="5" rx="8" ry="3" />
    <path d="M4 5v6c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 11v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6" />
  {:else if name === "palette"}
    <path d="M12 3a9 9 0 0 0 0 18h1.5a1.8 1.8 0 0 0 1.2-3.1 1.8 1.8 0 0 1 1.2-3.1H18A3 3 0 0 0 21 12a9 9 0 0 0-9-9Z" />
    <circle cx="7.5" cy="10" r=".8" fill="currentColor" stroke="none" />
    <circle cx="10" cy="6.8" r=".8" fill="currentColor" stroke="none" />
    <circle cx="14" cy="6.8" r=".8" fill="currentColor" stroke="none" />
  {:else if name === "shield"}
    <path d="M12 3 4.5 6v5.5c0 4.7 3.2 7.8 7.5 9.5 4.3-1.7 7.5-4.8 7.5-9.5V6L12 3Z" />
    <path d="m8.5 12 2.2 2.2 4.8-4.8" />
  {:else if name === "keyboard"}
    <rect x="2.5" y="5" width="19" height="14" rx="2.5" />
    <path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M10 13h.01M14 13h.01M18 13h.01M8 16h8" />
  {:else if name === "circle"}
    <circle cx="12" cy="12" r="8.5" />
  {:else if name === "x"}
    <path d="m6 6 12 12M18 6 6 18" />
  {/if}
</svg>

<style>
  .icon {
    display: block;
    flex: 0 0 auto;
  }
</style>
