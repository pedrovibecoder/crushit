import { invoke } from "@tauri-apps/api/core";
import type {
  Agent,
  AgentModel,
  AgentStatus,
  AnalysisSnapshot,
  Bootstrap,
  Category,
  CompletedTask,
  CategoryPatch,
  ExecutionSnapshot,
  RepoChanges,
  Stats,
  VerificationSnapshot,
  Goal,
  FocusSnapshot,
  NewTask,
  Project,
  ProjectInspection,
  RestSnapshot,
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

  listCategories: () => invoke<Category[]>("list_categories"),
  createCategory: (label: string, color: string) =>
    invoke<Category[]>("create_category", { label, color }),
  updateCategory: (categoryId: number, patch: CategoryPatch) =>
    invoke<Category[]>("update_category", { categoryId, patch }),
  deleteCategory: (categoryId: number) =>
    invoke<Category[]>("delete_category", { categoryId }),

  listTasks: (projectId: number) => invoke<Task[]>("list_tasks", { projectId }),
  completedTasks: (projectId: number, days: number) =>
    invoke<CompletedTask[]>("completed_tasks", { projectId, days }),
  createTask: (input: NewTask) => invoke<Task>("create_task", { input }),
  updateTask: (taskId: number, patch: TaskPatch) =>
    invoke<Task>("update_task", { taskId, patch }),
  deleteTask: (taskId: number) => invoke<void>("delete_task", { taskId }),
  reorderTasks: (projectId: number, orderedIds: number[]) =>
    invoke<Task[]>("reorder_tasks", { projectId, orderedIds }),
  suggestCriteria: (taskId: number) =>
    invoke<string[]>("suggest_criteria", { taskId }),
  setCriterionMet: (criterionId: number, isMet: boolean) =>
    invoke<Task>("set_criterion_met", { criterionId, isMet }),

  focusSnapshot: () => invoke<FocusSnapshot>("focus_snapshot"),
  startFocus: (taskId: number, durationMinutes?: number) =>
    invoke<FocusSnapshot>("start_focus", { taskId, durationMinutes }),
  pauseFocus: () => invoke<FocusSnapshot>("pause_focus"),
  resumeFocus: () => invoke<FocusSnapshot>("resume_focus"),
  stopFocus: () => invoke<FocusSnapshot>("stop_focus"),

  restSnapshot: () => invoke<RestSnapshot>("rest_snapshot"),
  startRest: (minutes: number) => invoke<RestSnapshot>("start_rest", { minutes }),
  stopRest: () => invoke<RestSnapshot>("stop_rest"),

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
  importTasks: (projectId: number, path: string) =>
    invoke<Goal>("import_tasks", { projectId, path }),
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
  openDesktopWindow: (view?: string) => invoke<void>("open_desktop_window", { view }),
  hideDesktopWindow: () => invoke<void>("hide_desktop_window"),
  resizePopup: (height: number) => invoke<void>("resize_popup", { height }),
  quitApp: () => invoke<void>("quit_app"),
};
