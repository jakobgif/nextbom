import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Project } from "@/types/Project";
import type { ProjectState } from "@/types/ProjectState";

interface ProjectStore {
  project: Project | null;
  hasUnsavedChanges: boolean;
  /** Call once from the root component to bootstrap state and subscribe to backend events. */
  initialize: () => Promise<void>;
}

// Module-level guard so re-renders never re-subscribe.
let _initialized = false;

export const useProjectStore = create<ProjectStore>((set) => ({
  project: null,
  hasUnsavedChanges: false,
  initialize: async () => {
    if (_initialized) return;
    _initialized = true;

    const state = await invoke<ProjectState>("get_project_state");
    set({ project: state.project, hasUnsavedChanges: state.has_unsaved_changes });

    listen<ProjectState>("project-changed", (event) => {
      set({
        project: event.payload.project,
        hasUnsavedChanges: event.payload.has_unsaved_changes,
      });
    });
  },
}));