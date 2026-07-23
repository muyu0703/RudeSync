import type { Task } from "../../types";

type CompletionTask = Pick<Task, "projectId" | "status" | "subtasks">;

export function unfinishedSubtaskCount(
  task: Pick<CompletionTask, "subtasks">,
): number {
  return task.subtasks.filter((subtask) => !subtask.completed).length;
}

export function needsSubtaskCompletionConfirmation(
  task: Pick<CompletionTask, "subtasks" | "status">,
  completing: boolean,
): boolean {
  return (
    completing &&
    task.status !== "completed" &&
    unfinishedSubtaskCount(task) > 0
  );
}

export function shouldOfferCompletedWork(
  task: Pick<CompletionTask, "projectId" | "status">,
  completing: boolean,
): boolean {
  return (
    completing &&
    task.status !== "completed" &&
    Boolean(task.projectId)
  );
}
