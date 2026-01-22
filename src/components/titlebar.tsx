import { ListPlus, Minus, Square, X } from "lucide-react";
import { Button } from "./ui/button";
import { Menubar, MenubarCheckboxItem, MenubarContent, MenubarItem, MenubarMenu, MenubarSeparator, MenubarShortcut, MenubarSub, MenubarSubContent, MenubarSubTrigger, MenubarTrigger } from "./ui/menubar";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Project } from "@/types/Project";
import { NewProjectDialog } from "./new-project";

export function Titlebar(){
  const appWindow = getCurrentWindow();

  const [currentProject, setCurrentProject] = useState<Project | null>(null);
  const [newProjectDialogOpen, setNewProjectDialogOpen] = useState(false);

  useEffect(() => {
    // Listen for project changes from backend
    listen<Project | null>("project-changed", (event) => {
      setCurrentProject(event.payload);
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
                <MenubarItem disabled>Save Project</MenubarItem>
                <MenubarItem onClick={() => setNewProjectDialogOpen(true)}>New Project</MenubarItem>
                <MenubarItem disabled>Open Project</MenubarItem>
                <MenubarSub>
                  <MenubarSubTrigger>Open Recent</MenubarSubTrigger>
                  <MenubarSubContent>
                    <MenubarItem disabled>File</MenubarItem>
                  </MenubarSubContent>
                </MenubarSub>
                <MenubarItem disabled={!currentProject} onClick={() => invoke("close_project")}>Close Project</MenubarItem>
                <MenubarSeparator />
                <MenubarItem disabled>Toggle Theme</MenubarItem>
                <MenubarSeparator />
                <MenubarItem disabled>Restart</MenubarItem>
                <MenubarItem disabled>Exit</MenubarItem>
              </MenubarContent>
            </MenubarMenu>
            <MenubarMenu>
              <MenubarTrigger>
                <p>Edit</p>
              </MenubarTrigger>
              <MenubarContent>
                <MenubarItem>
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
                <MenubarItem>Paste</MenubarItem>
              </MenubarContent>
            </MenubarMenu>
            <MenubarMenu>
              <MenubarTrigger>
                <p>Help</p>
              </MenubarTrigger>
              <MenubarContent>
                <MenubarItem inset disabled>Report Issue</MenubarItem>
                <MenubarSeparator />
                <MenubarCheckboxItem disabled>Development Mode</MenubarCheckboxItem>
                <MenubarSeparator />
                <MenubarItem inset disabled>About</MenubarItem>
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
          </div>
        <div className="absolute left-1/2 -translate-x-1/2">
          <p className="text-muted-foreground text-sm">{currentProject?.title}</p>
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