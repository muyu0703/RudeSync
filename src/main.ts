import { mount } from "svelte";
import App from "./App.svelte";
// Self-hosted so the app keeps its typography offline. On macOS the stack
// prefers the system SF Pro and never loads this file.
import "@fontsource-variable/inter";
import "./app.css";

const target = document.getElementById("app");

if (!target) {
  throw new Error("RudeSync could not find its application root.");
}

// The window is painted by the OS (Mica on Windows 11, vibrancy on macOS).
// Only then do we let the shell go transparent so the backdrop shows through.
if ("__TAURI_INTERNALS__" in window || "__TAURI__" in window) {
  document.body.classList.add("has-window-effect");
}

mount(App, { target });
