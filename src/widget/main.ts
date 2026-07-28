import { mount } from "svelte";
import TaskWidget from "./TaskWidget.svelte";
import { applyMotionPreference } from "../lib/motion";
import "./widget.css";

const target = document.getElementById("widget");

if (!target) {
  throw new Error("RudeSync could not find its task-widget root.");
}

// The widget is its own webview, so it reads the shared preference itself.
applyMotionPreference();

mount(TaskWidget, { target });
