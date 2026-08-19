import { invoke } from "@tauri-apps/api/core";
import type {
  Agent,
  AgentModel,
  AgentStatus,
  AnalysisSnapshot,
  Bootstrap,
  ExecutionSnapshot,
  RepoChanges,
  Stats,
  VerificationSnapshot,
  Goal,
  FocusSnapshot,
  NewTask,
  Project,
  ProjectInspection,
  Settings,
  SettingsPatch,
  ShortcutInfo,
  Task,
  TaskPatch,
} from "../types";

/** Every backend error arrives as a plain string. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

export const ipc = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),

  listProjects: () => invoke<Project[]>("list_projects"),
  inspectDirectory: (path: string) =>
    invoke<ProjectInspection>("inspect_directory", { path }),
  addProject: (path: string) => invoke<Project>("add_project", { path }),
  setActiveProject: (projectId: number | null) =>
    invoke<Settings>("set_active_project", { projectId }),
  removeProject: (projectId: number) =>
    invoke<Project[]>("remove_project", { projectId }),
  refreshProject: (projectId: number) =>
    invoke<Project>("refresh_project", { projectId }),

  listTasks: (projectId: number) => invoke<Task[]>("list_tasks", { projectId }),
  createTask: (input: NewTask) => invoke<Task>("create_task", { input }),
  updateTask: (taskId: number, patch: TaskPatch) =>
    invoke<Task>("update_task", { taskId, patch }),
  deleteTask: (taskId: number) => invoke<void>("delete_task", { taskId }),
  reorderTasks: (projectId: number, orderedIds: number[]) =>
    invoke<Task[]>("reorder_tasks", { projectId, orderedIds }),
  setCriterionMet: (criterionId: number, isMet: boolean) =>
    invoke<Task>("set_criterion_met", { criterionId, isMet }),

  focusSnapshot: () => invoke<FocusSnapshot>("focus_snapshot"),
  startFocus: (taskId: number, durationMinutes?: number) =>
    invoke<FocusSnapshot>("start_focus", { taskId, durationMinutes }),
  pauseFocus: () => invoke<FocusSnapshot>("pause_focus"),
  resumeFocus: () => invoke<FocusSnapshot>("resume_focus"),
  stopFocus: () => invoke<FocusSnapshot>("stop_focus"),

  performanceStats: () => invoke<Stats>("performance_stats"),

  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (patch: SettingsPatch) =>
    invoke<Settings>("update_settings", { patch }),

  agentStatus: (agent: Agent, force = false) =>
    invoke<AgentStatus>("agent_status", { agent, force }),
  agentModels: (agent: Agent) => invoke<AgentModel[]>("agent_models", { agent }),

  latestGoal: (projectId: number) =>
    invoke<Goal | null>("latest_goal", { projectId }),
  analysisSnapshot: () => invoke<AnalysisSnapshot>("analysis_snapshot"),
  startAnalysis: (projectId: number, prompt: string) =>
    invoke<Goal>("start_analysis", { projectId, prompt }),
  cancelAnalysis: () => invoke<AnalysisSnapshot>("cancel_analysis"),
  acceptPlan: (goalId: number) => invoke<Task[]>("accept_plan", { goalId }),
  discardPlan: (goalId: number) => invoke<void>("discard_plan", { goalId }),

  executionSnapshot: () => invoke<ExecutionSnapshot>("execution_snapshot"),
  taskHasThread: (taskId: number) => invoke<boolean>("task_has_thread", { taskId }),
  startExecution: (taskId: number, resume = false) =>
    invoke<Task>("start_execution", { taskId, resume }),
  stopExecution: () => invoke<ExecutionSnapshot>("stop_execution"),
  respondToApproval: (approve: boolean) =>
    invoke<ExecutionSnapshot>("respond_to_approval", { approve }),
  clearExecution: () => invoke<ExecutionSnapshot>("clear_execution"),

  repoChanges: () => invoke<RepoChanges>("repo_changes"),
  fileDiff: (path: string) => invoke<string>("file_diff", { path }),

  verificationSnapshot: () => invoke<VerificationSnapshot>("verification_snapshot"),
  startVerification: (taskId: number) =>
    invoke<VerificationSnapshot>("start_verification", { taskId }),
  cancelVerification: () => invoke<VerificationSnapshot>("cancel_verification"),

  popupShortcut: () => invoke<ShortcutInfo>("popup_shortcut"),

  hidePopup: () => invoke<void>("hide_popup"),
  openDesktopWindow: () => invoke<void>("open_desktop_window"),
  hideDesktopWindow: () => invoke<void>("hide_desktop_window"),
  resizePopup: (height: number) => invoke<void>("resize_popup", { height }),
  quitApp: () => invoke<void>("quit_app"),
};
