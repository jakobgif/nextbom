import { ListPlus, Minus, Square, X } from "lucide-react";
import { Button } from "./ui/button";
import { Menubar, MenubarCheckboxItem, MenubarContent, MenubarItem, MenubarMenu, MenubarSeparator, MenubarShortcut, MenubarSub, MenubarSubContent, MenubarSubTrigger, MenubarTrigger } from "./ui/menubar";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Project } from "@/types/Project";
import { ProjectState } from "@/types/ProjectState";
import { NewProjectDialog } from "./new-project";
import { toast } from "sonner";
import { useTheme } from "./theme-provider";
import { relaunch, exit } from "@tauri-apps/plugin-process";

export function Titlebar(){
  const appWindow = getCurrentWindow();
  const { theme, setTheme } = useTheme();

  const [currentProject, setCurrentProject] = useState<Project | null>(null);
  const [currentProjectUnsaved, setCurrentProjectUnsaved] = useState<boolean>(false);
  const [newProjectDialogOpen, setNewProjectDialogOpen] = useState(false);

  useEffect(() => {
    // Initialize project state on mount
    invoke<ProjectState>("get_project_state").then((state) => {
      setCurrentProject(state.project);
      setCurrentProjectUnsaved(state.has_unsaved_changes);
    });

    // Listen for project changes from backend
    listen<ProjectState>("project-changed", (event) => {
      setCurrentProject(event.payload.project);
      setCurrentProjectUnsaved(event.payload.has_unsaved_changes);
    });
  }, []);

  return (
    <>
      <div className="flex flex-row items-center bg-card select-none shadow-2xl z-50" data-tauri-drag-region>
        <div className="flex flex-row items-center">
          <ListPlus className="size-icon mx-2 text-primary"/>
          <Menubar>
            <MenubarMenu>
              <MenubarTrigger>
                <p>File</p>
              </MenubarTrigger>
              <MenubarContent>
                <MenubarItem onClick={() => setNewProjectDialogOpen(true)}>New Project</MenubarItem>
                <MenubarSeparator />
                <MenubarItem disabled={!currentProject} onClick={async () => {
                  try {
                    await invoke("save_project");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Save Project</MenubarItem>
                <MenubarItem disabled={!currentProject} onClick={async () => {
                  try {
                    await invoke("save_project", { saveAs: true });
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Save Project As</MenubarItem>
                <MenubarSeparator />
                <MenubarItem onClick={async () => {
                  try {
                    await invoke("open_project");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Open Project</MenubarItem>
                <MenubarSub>
                  <MenubarSubTrigger>Open Recent</MenubarSubTrigger>
                  <MenubarSubContent>
                    <MenubarItem disabled>File</MenubarItem>
                  </MenubarSubContent>
                </MenubarSub>
                <MenubarSeparator />
                <MenubarItem disabled={!currentProject} onClick={() => invoke("close_project")}>Close Project</MenubarItem>
                <MenubarSeparator />
                <MenubarItem onClick={async () => {
                  try {
                    await relaunch();
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Restart</MenubarItem>
                <MenubarItem onClick={async () => {
                  try {
                    await exit(0);
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Exit</MenubarItem>
              </MenubarContent>
            </MenubarMenu>
            <MenubarMenu>
              <MenubarTrigger>
                <p>Edit</p>
              </MenubarTrigger>
              <MenubarContent>
                {/* <MenubarItem>
                  Undo <MenubarShortcut>⌘Z</MenubarShortcut>
                </MenubarItem>
                <MenubarItem>
                  Redo <MenubarShortcut>⇧⌘Z</MenubarShortcut>
                </MenubarItem>
                <MenubarSeparator />
                <MenubarSub>
                  <MenubarSubTrigger>Find</MenubarSubTrigger>
                  <MenubarSubContent>
                    <MenubarItem>Search the web</MenubarItem>
                    <MenubarSeparator />
                    <MenubarItem>Find...</MenubarItem>
                    <MenubarItem>Find Next</MenubarItem>
                    <MenubarItem>Find Previous</MenubarItem>
                  </MenubarSubContent>
                </MenubarSub>
                <MenubarSeparator />
                <MenubarItem>Cut</MenubarItem>
                <MenubarItem>Copy</MenubarItem>
                <MenubarItem>Paste</MenubarItem> */}
              </MenubarContent>
            </MenubarMenu>
            <MenubarMenu>
              <MenubarTrigger>
                <p>Help</p>
              </MenubarTrigger>
              <MenubarContent>
                <MenubarItem inset onClick={() => {
                  setTheme(theme === "dark" ? "light" : "dark");
                }}>Toggle Theme</MenubarItem>
                {/* <MenubarSeparator />
                <MenubarItem inset disabled>Report Issue</MenubarItem>
                <MenubarSeparator />
                <MenubarCheckboxItem disabled>Development Mode</MenubarCheckboxItem>
                <MenubarSeparator />
                <MenubarItem inset disabled>About</MenubarItem> */}
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
          </div>
        <div className="absolute left-1/2 -translate-x-1/2">
          <p className="text-muted-foreground text-sm">
            {currentProjectUnsaved && "(unsaved) "}
            {currentProject?.title}
          </p>
        </div>
        <div className="ml-auto flex flex-row">
          <Button variant={"ghost"} size={"icon"} className="rounded-none" onClick={() => appWindow.minimize()}><Minus className="size-4"/></Button>
          <Button variant={"ghost"} size={"icon"} className="rounded-none" onClick={() => appWindow.toggleMaximize()}><Square className="size-3.5"/></Button>
          <Button variant={"ghost"} size={"icon"} className="rounded-none" onClick={() => appWindow.close()}><X className="size-4.5"/></Button>
        </div>
      </div>
      <NewProjectDialog open={newProjectDialogOpen} onOpenChange={setNewProjectDialogOpen} />
    </>
  )
}