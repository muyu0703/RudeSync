import { mount } from "svelte";
import TaskWidget from "./TaskWidget.svelte";
import "./widget.css";

const target = document.getElementById("widget");

if (!target) {
  throw new Error("RudeSync could not find its task-widget root.");
}

mount(TaskWidget, { target });
