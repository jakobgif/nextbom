import { useState, useEffect } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { Button } from "./ui/button";
import { applyCustomTheme, resetCustomTheme, getSavedThemeCss } from "@/lib/custom-theme";

interface CustomThemeDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CustomThemeDialog({ open, onOpenChange }: CustomThemeDialogProps) {
  const [value, setValue] = useState("");
  const [error, setError] = useState(false);
  const [hasSaved, setHasSaved] = useState(false);

  useEffect(() => {
    if (open) {
      const saved = getSavedThemeCss();
      setValue(saved ?? "");
      setHasSaved(saved !== null);
      setError(false);
    }
  }, [open]);

  const handleApply = () => {
    const ok = applyCustomTheme(value);
    if (!ok) {
      setError(true);
      return;
    }
    setHasSaved(true);
    onOpenChange(false);
  };

  const handleReset = () => {
    resetCustomTheme();
    setValue("");
    setHasSaved(false);
    setError(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Customize Theme</DialogTitle>
          <DialogDescription>
            Create a theme at tweakcn.com/editor/theme, then paste the exported CSS below.
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-2">
          <textarea
            className="placeholder:text-muted-foreground dark:bg-input/30 border-input w-full rounded-md border bg-transparent px-3 py-2 text-sm font-mono shadow-xs outline-none resize-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] h-72"
            placeholder={`:root {\n  --primary: rgb(59, 130, 246);\n  ...\n}\n.dark {\n  --primary: rgb(96, 165, 250);\n  ...\n}`}
            value={value}
            onChange={(e) => { setValue(e.target.value); setError(false); }}
            spellCheck={false}
          />
          {error && (
            <p className="text-sm text-destructive">No valid :root or .dark blocks found in the pasted CSS.</p>
          )}
        </div>
        <DialogFooter className="flex-row items-center">
          {hasSaved && (
            <Button variant="outline" onClick={handleReset} className="mr-auto">
              Reset to Default
            </Button>
          )}
          <Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button onClick={handleApply} disabled={!value.trim()}>Apply</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
