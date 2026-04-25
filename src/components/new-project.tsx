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
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { toast } from "sonner";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

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
  const [specificsOpen, setSpecificsOpen] = useState(false);

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
      setSpecificsOpen(false);
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

  const specificsOptions = [
    { value: "", label: "None" },
    ...availableSpecifics.map((t) => ({ value: t, label: t.replace(/^alt_/, "") })),
  ];

  const selectedLabel = specificsOptions.find((o) => o.value === projectSpecifics)?.label ?? "None";

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
            <Popover open={specificsOpen} onOpenChange={setSpecificsOpen}>
              <PopoverTrigger asChild>
                <button
                  type="button"
                  disabled={!databasePath}
                  className={cn(
                    "border-input dark:bg-input/30 flex h-9 w-full items-center justify-between rounded-md border bg-transparent px-3 py-1 text-sm shadow-xs outline-none",
                    "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
                    "disabled:cursor-not-allowed disabled:opacity-50",
                    !databasePath && "text-muted-foreground",
                  )}
                >
                  <span>{databasePath ? selectedLabel : "Select a database first"}</span>
                  <ChevronDown className="size-4 opacity-50" />
                </button>
              </PopoverTrigger>
              <PopoverContent className="w-[--radix-popover-trigger-width] p-1" align="start">
                {specificsOptions.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => { setProjectSpecifics(opt.value); setSpecificsOpen(false); }}
                    className="hover:bg-accent hover:text-accent-foreground flex w-full items-center gap-2 rounded-xs px-2 py-1.5 text-sm outline-none"
                  >
                    <Check className={cn("size-4", projectSpecifics === opt.value ? "opacity-100" : "opacity-0")} />
                    {opt.label}
                  </button>
                ))}
              </PopoverContent>
            </Popover>
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
