import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
  const [projectSpecifics, setProjectSpecifics] = useState("");
  const [designVariant, setDesignVariant] = useState("");

  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : internalOpen;
  const setOpen = isControlled ? onOpenChange! : setInternalOpen;

  const handleCreate = async () => {
    if (!title.trim()) {
      return;
    }

    try {
      await invoke("create_project", {
        title: title.trim(),
        engineer: engineer.trim() || null,
        projectSpecifics: projectSpecifics.trim() || null,
        designVariant: designVariant.trim() || null,
      });
      setOpen(false);
    } catch (error: any) {
      console.error("Failed to create project:", error);
      toast.error(error.toString())
    }
  };

  const handleOpenChange = (open: boolean) => {
    setOpen(open);
    if (!open) {
      setTitle("");
      setEngineer("");
      setProjectSpecifics("");
      setDesignVariant("");
    }
  };

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
            <Label>Project Specifics (Optional)</Label>
            <Input
              id="project-specifics"
              placeholder="parts_2025"
              value={projectSpecifics}
              onChange={(e) => setProjectSpecifics(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label>Design Variant (Optional)</Label>
            <Input
              id="design-variant"
              placeholder="e.g. full, lite"
              value={designVariant}
              onChange={(e) => setDesignVariant(e.target.value)}
            />
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