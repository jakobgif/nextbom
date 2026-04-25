import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openFilePicker } from "@tauri-apps/plugin-dialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { toast } from "sonner";

interface NewProjectDialogProps {
  trigger?: React.ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function NewProjectDialog({ trigger, open: controlledOpen, onOpenChange }: NewProjectDialogProps) {
  const [internalOpen, setInternalOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [engineer, setEngineer] = useState("");
  const [databasePath, setDatabasePath] = useState("");
  const [availableSpecifics, setAvailableSpecifics] = useState<string[]>([]);
  const [projectSpecifics, setProjectSpecifics] = useState("");

  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : internalOpen;
  const setOpen = isControlled ? onOpenChange! : setInternalOpen;

  useEffect(() => {
    if (open) {
      setTitle("");
      setEngineer("");
      setDatabasePath("");
      setAvailableSpecifics([]);
      setProjectSpecifics("");
    }
  }, [open]);

  const handlePickDatabase = async () => {
    const selected = await openFilePicker({
      title: "Select Parts Database File",
      filters: [{ name: "Parts Database", extensions: ["nextdb"] }],
      multiple: false,
      directory: false,
    });
    if (!selected) return;
    const path = selected as string;
    setDatabasePath(path);
    setProjectSpecifics("");
    try {
      const tables = await invoke<string[]>("get_parts_tables_from_path", { databasePath: path });
      setAvailableSpecifics(tables);
    } catch {
      setAvailableSpecifics([]);
    }
  };

  const handleCreate = async () => {
    if (!title.trim()) return;

    try {
      await invoke("create_project", {
        title: title.trim(),
        engineer: engineer.trim() || null,
        projectSpecifics: projectSpecifics || null,
        designVariant: null,
        databasePath: databasePath || null,
      });
      setOpen(false);
    } catch (error: any) {
      console.error("Failed to create project:", error);
      toast.error(error.toString());
    }
  };

  const handleOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
  };

  const dbFilename = databasePath
    ? databasePath.replace(/\\/g, "/").split("/").pop()
    : "";

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      {trigger && (
        <DialogTrigger asChild>
          {trigger}
        </DialogTrigger>
      )}
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create New Project</DialogTitle>
          <DialogDescription>
            Create a new project. Enter a project title to get started.
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label>Project Title</Label>
            <Input
              id="project-title"
              placeholder="complex_pcb_v1"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              autoFocus
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label>Engineer (Optional)</Label>
            <Input
              id="engineer"
              placeholder="John Doe"
              value={engineer}
              onChange={(e) => setEngineer(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label>Parts Database (Optional)</Label>
            <div className="flex gap-2">
              <Input
                readOnly
                value={dbFilename}
                placeholder="No database selected"
                className="flex-1 cursor-default"
              />
              <Button type="button" variant="outline" onClick={handlePickDatabase}>
                Browse
              </Button>
            </div>
          </div>
          <div className="flex flex-col gap-2">
            <Label>Project Specifics (Optional)</Label>
            <select
              value={projectSpecifics}
              onChange={(e) => setProjectSpecifics(e.target.value)}
              disabled={!databasePath}
              className="border-input dark:bg-input/30 h-9 w-full rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none disabled:cursor-not-allowed disabled:opacity-50 focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]"
            >
              <option value="">
                {databasePath ? "None" : "Select a database first"}
              </option>
              {availableSpecifics.map((table) => (
                <option key={table} value={table}>
                  {table.replace(/^alt_/, "")}
                </option>
              ))}
            </select>
          </div>
        </div>
        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => handleOpenChange(false)}
          >
            Cancel
          </Button>
          <Button
            onClick={handleCreate}
            disabled={!title.trim()}
          >
            Create Project
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
