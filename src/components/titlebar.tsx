import { ListPlus, Minus, Square, X } from "lucide-react";
import { Button } from "./ui/button";
import { Menubar, MenubarContent, MenubarItem, MenubarMenu, MenubarSeparator, MenubarShortcut, MenubarSub, MenubarSubContent, MenubarSubTrigger, MenubarTrigger } from "./ui/menubar";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { NewProjectDialog } from "./new-project";
import { toast } from "sonner";
import { useTheme } from "./theme-provider";
import { useTooltipSettings } from "./ui/tooltip";
import { relaunch } from "@tauri-apps/plugin-process";
import { SetStringDialog } from "./set-string-dialog";
import { useProjectStore } from "@/store/project-store";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";

export function Titlebar(){
  const appWindow = getCurrentWindow();
  const { theme, setTheme } = useTheme();
  const { enabled: tooltipsEnabled, toggle: toggleTooltips } = useTooltipSettings();
  const { project, hasUnsavedChanges, recentProjects } = useProjectStore();

  const [newProjectDialogOpen, setNewProjectDialogOpen] = useState(false);
  const [titleDialogOpen, setTitleDialogOpen] = useState(false);
  const [engineerDialogOpen, setEngineerDialogOpen] = useState(false);
  const [designVariantDialogOpen, setDesignVariantDialogOpen] = useState(false);
  const [partsAlternatives, setPartsAlternatives] = useState<string[]>([]);

  useEffect(() => {
    if (!project?.database_path) { setPartsAlternatives([]); return; }
    invoke<string[]>("get_parts_tables")
      .then(setPartsAlternatives)
      .catch(() => setPartsAlternatives([]));
  }, [project?.database_path]);
  // Stores the action to run after the user confirms discarding unsaved changes.
  const [pendingAction, setPendingAction] = useState<(() => void) | null>(null);

  // Ref so the window close handler always reads the latest value without re-registering.
  const hasUnsavedChangesRef = useRef(hasUnsavedChanges);
  useEffect(() => {
    hasUnsavedChangesRef.current = hasUnsavedChanges;
  }, [hasUnsavedChanges]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        if (!project) return;
        invoke("save_project").catch((error: any) => {
          toast.error(error.toString());
        });
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [project]);

  // Intercept window close when there are unsaved changes.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    appWindow.onCloseRequested((event) => {
      if (hasUnsavedChangesRef.current) {
        event.preventDefault();
        setPendingAction(() => () => appWindow.destroy());
      }
    }).then((fn) => { unlisten = fn; });
    return () => unlisten?.();
  }, []);

  /** Runs `action` immediately if there are no unsaved changes, otherwise prompts first. */
  const withUnsavedCheck = (action: () => void) => {
    if (hasUnsavedChanges) {
      setPendingAction(() => action);
    } else {
      action();
    }
  };

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
                <MenubarItem onClick={() => withUnsavedCheck(() => setNewProjectDialogOpen(true))}>New Project</MenubarItem>
                <MenubarSeparator />
                <MenubarItem disabled={!project} onClick={async () => {
                  try {
                    await invoke("save_project");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Save Project<MenubarShortcut>Ctrl+S</MenubarShortcut></MenubarItem>
                <MenubarItem disabled={!project} onClick={async () => {
                  try {
                    await invoke("save_project", { saveAs: true });
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Save Project As</MenubarItem>
                <MenubarSeparator />
                <MenubarItem onClick={() => withUnsavedCheck(async () => {
                  try {
                    await invoke("open_project");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                })}>Open Project</MenubarItem>
                <MenubarSub>
                  <MenubarSubTrigger>Open Recent</MenubarSubTrigger>
                  <MenubarSubContent>
                    {recentProjects.length === 0
                      ? <MenubarItem disabled>No recent projects</MenubarItem>
                      : recentProjects.slice(0, 10).map((rp) => (
                          <MenubarItem key={rp.file_path} onClick={() => withUnsavedCheck(async () => {
                            try {
                              await invoke("open_project", { path: rp.file_path });
                            } catch (error: any) {
                              toast.error(error.toString());
                            }
                          })}>
                            {rp.title ?? rp.file_path}
                          </MenubarItem>
                        ))
                    }
                    <MenubarSeparator />
                    <MenubarItem onClick={async () => {
                      try {
                        await invoke("clear_recent_projects");
                      } catch (error: any) {
                        toast.error(error.toString());
                      }
                    }}>Clear Recent</MenubarItem>
                  </MenubarSubContent>
                </MenubarSub>
                <MenubarSeparator />
                <MenubarItem disabled={!project} onClick={() => withUnsavedCheck(async () => {
                  try {
                    await invoke("close_project");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                })}>Close Project</MenubarItem>
                <MenubarSeparator />
                <MenubarItem onClick={async () => {
                  try {
                    await relaunch();
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Restart</MenubarItem>
                <MenubarItem onClick={() => withUnsavedCheck(() => appWindow.destroy())}>Exit</MenubarItem>
              </MenubarContent>
            </MenubarMenu>
            <MenubarMenu>
              <MenubarTrigger>
                <p>Project</p>
              </MenubarTrigger>
              <MenubarContent>
                <MenubarItem disabled={!project} onSelect={() => setTitleDialogOpen(true)}>Set Title</MenubarItem>
                <MenubarItem disabled={!project} onSelect={() => setEngineerDialogOpen(true)}>Set Engineer</MenubarItem>
                <MenubarSeparator />
                <MenubarItem disabled={!project} onClick={async () => {
                  try {
                    await invoke("set_database_path");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>Select Parts Database</MenubarItem>
                <MenubarSub>
                  <MenubarSubTrigger disabled={!project}>Set Project Specifics</MenubarSubTrigger>
                  <MenubarSubContent>
                    <MenubarItem onSelect={async () => {
                      try { await invoke("set_project_specifics", { projectSpecifics: "" }); }
                      catch (e: any) { toast.error(e.toString()); }
                    }}>None</MenubarItem>
                    {partsAlternatives.length > 0 && <MenubarSeparator />}
                    {partsAlternatives.map((table) => (
                      <MenubarItem key={table} onSelect={async () => {
                        try { await invoke("set_project_specifics", { projectSpecifics: table }); }
                        catch (e: any) { toast.error(e.toString()); }
                      }}>{table.replace(/^alt_/, "")}</MenubarItem>
                    ))}
                    {!project?.database_path && <MenubarItem disabled>No database linked</MenubarItem>}
                  </MenubarSubContent>
                </MenubarSub>
                <MenubarItem disabled={!project} onSelect={() => setDesignVariantDialogOpen(true)}>Set Design Variant</MenubarItem>
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
                <MenubarItem onClick={toggleTooltips}>
                  {tooltipsEnabled ? "Disable Tooltips" : "Enable Tooltips"}
                </MenubarItem>
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
        </div>
        <div className="absolute left-1/2 -translate-x-1/2">
          <p className="text-muted-foreground text-sm">
            {hasUnsavedChanges && "(unsaved) "}
            {project?.title}
          </p>
        </div>
        <div className="ml-auto flex flex-row">
          <Button variant={"ghost"} size={"icon"} className="rounded-none" onClick={() => appWindow.minimize()}><Minus className="size-4"/></Button>
          <Button variant={"ghost"} size={"icon"} className="rounded-none" onClick={() => appWindow.toggleMaximize()}><Square className="size-3.5"/></Button>
          <Button variant={"ghost"} size={"icon"} className="rounded-none" onClick={() => withUnsavedCheck(() => appWindow.destroy())}><X className="size-4.5"/></Button>
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
        currentValue={project?.title || ""}
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
        currentValue={project?.engineer || ""}
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
        open={designVariantDialogOpen}
        onOpenChange={setDesignVariantDialogOpen}
        title="Set Design Variant"
        description="Enter the design variant identifier for this project (e.g. full, lite)."
        label="Design Variant"
        placeholder="e.g. full, lite"
        currentValue={project?.design_variant || ""}
        onSubmit={async (value) => {
          try {
            await invoke("set_design_variant", { designVariant: value });
            setDesignVariantDialogOpen(false);
          } catch (error: any) {
            toast.error(error.toString());
          }
        }}
      />

      {/* Unsaved changes confirmation */}
      <Dialog open={pendingAction !== null} onOpenChange={(open) => { if (!open) setPendingAction(null); }}>
        <DialogContent showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Unsaved Changes</DialogTitle>
            <DialogDescription>
              You have unsaved changes. Do you want to discard them and continue?
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setPendingAction(null)}>Cancel</Button>
            <Button variant="destructive" onClick={() => {
              const action = pendingAction;
              setPendingAction(null);
              action?.();
            }}>Discard Changes</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
