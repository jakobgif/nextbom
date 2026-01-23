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
import { SetStringDialog } from "./set-string-dialog";

export function Titlebar(){
  const appWindow = getCurrentWindow();
  const { theme, setTheme } = useTheme();

  const [currentProject, setCurrentProject] = useState<Project | null>(null);
  const [currentProjectUnsaved, setCurrentProjectUnsaved] = useState<boolean>(false);
  const [newProjectDialogOpen, setNewProjectDialogOpen] = useState(false);
  const [titleDialogOpen, setTitleDialogOpen] = useState(false);
  const [engineerDialogOpen, setEngineerDialogOpen] = useState(false);
  const [projectSpecificsDialogOpen, setProjectSpecificsDialogOpen] = useState(false);

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

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        if (!currentProject) return;
        invoke("save_project").catch((error: any) => {
          toast.error(error.toString());
        });
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [currentProject]);

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
                }}>Save Project<MenubarShortcut>Ctrl+S</MenubarShortcut></MenubarItem>
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
                  {/* <MenubarSubTrigger>Open Recent</MenubarSubTrigger> */}
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
                <MenubarItem disabled={!currentProject} onSelect={() => setTitleDialogOpen(true)}>Set Title</MenubarItem>
                <MenubarItem disabled={!currentProject} onSelect={() => setEngineerDialogOpen(true)}>Set Engineer</MenubarItem>
                <MenubarSeparator />
                <MenubarItem disabled={!currentProject} onClick={async () => {
                  try {
                    await invoke("set_database_path");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Select Database</MenubarItem>
                <MenubarItem disabled={!currentProject} onSelect={() => setProjectSpecificsDialogOpen(true)}>Set Project Specifics</MenubarItem>
              </MenubarContent>
            </MenubarMenu>
            <MenubarMenu>
              <MenubarTrigger>
                <p>Help</p>
              </MenubarTrigger>
              <MenubarContent>
                <MenubarItem onClick={() => {
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
      <SetStringDialog
        open={titleDialogOpen}
        onOpenChange={setTitleDialogOpen}
        title="Set Project Title"
        description="Enter a new title for your project."
        label="Title"
        placeholder="Enter project title"
        currentValue={currentProject?.title || ""}
        onSubmit={async (value) => {
          try {
            await invoke("set_project_title", { title: value });
            setTitleDialogOpen(false);
          } catch (error: any) {
            toast.error(error.toString());
          }
        }}
      />
      <SetStringDialog
        open={engineerDialogOpen}
        onOpenChange={setEngineerDialogOpen}
        title="Set Project Engineer"
        description="Enter the name of the engineer working on the project."
        label="Engineer"
        placeholder="Enter engineer name"
        currentValue={currentProject?.engineer || ""}
        onSubmit={async (value) => {
          try {
            await invoke("set_project_engineer", { engineer: value });
            setEngineerDialogOpen(false);
          } catch (error: any) {
            toast.error(error.toString());
          }
        }}
      />
      <SetStringDialog
        open={projectSpecificsDialogOpen}
        onOpenChange={setProjectSpecificsDialogOpen}
        title="Set Project Specifics"
        description="Enter the identifier of the project specific parts to use for this project."
        label="Project Specifics"
        placeholder="Enter identifier"
        currentValue={currentProject?.project_specifics || ""}
        onSubmit={async (value) => {
          try {
            await invoke("set_project_specifics", { projectSpecifics: value });
            setProjectSpecificsDialogOpen(false);
          } catch (error: any) {
            toast.error(error.toString());
          }
        }}
      />
    </>
  )
}