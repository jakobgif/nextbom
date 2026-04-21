import "./App.css";
import { useEffect, useState } from "react";
import { Check, Info, ListPlus } from "lucide-react";
import { Titlebar } from "./components/titlebar";
import { Button } from "./components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "./components/ui/empty";
import { Input } from "./components/ui/input";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "./components/ui/accordion";
import { Popover, PopoverContent, PopoverTrigger } from "./components/ui/popover";
import { Field, FieldError, FieldLabel } from "./components/ui/field";
import { Checkbox } from "./components/ui/checkbox";
import { NewProjectDialog } from "./components/new-project";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { useProjectStore } from "./store/project-store";
import { formatLastChange } from "./lib/utils";

function App() {
  const { project, recentProjects, initialize } = useProjectStore();

  useEffect(() => {
    initialize();
  }, []);

  return (
    <div className="h-screen flex flex-col overflow-clip">
      <Titlebar/>
      <div className="flex flex-col flex-1 overflow-clip">
        {!project ? (
          <Empty className="select-none">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <ListPlus className="text-primary"/>
              </EmptyMedia>
              <EmptyTitle>nextbom</EmptyTitle>
              <EmptyDescription>
                You haven&apos;t loaded a project yet. Get started by creating or opening one.
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <div className="flex gap-2">
                <NewProjectDialog trigger={
                  <Button>Create Project</Button>
                } />
                <Button variant="outline" onClick={async () => {
                  try {
                    await invoke("open_project");
                  } catch (error: any) {
                    toast.error(error.toString());
                  }
                }}>
                  Open Project
                </Button>
              </div>
              {recentProjects.length > 0 && (
                <div className="flex flex-col gap-1 mt-4 w-full max-w-xs">
                  <p className="text-xs text-muted-foreground px-2 mb-1">Recent</p>
                  {recentProjects.slice(0, 6).map((rp) => (
                    <Button
                      key={rp.file_path}
                      variant="ghost"
                      className="justify-start h-auto py-1.5 px-2 text-left"
                      onClick={async () => {
                        try {
                          await invoke("open_project", { path: rp.file_path });
                        } catch (error: any) {
                          toast.error(error.toString());
                        }
                      }}
                    >
                      <span className="truncate text-sm">{rp.title ?? rp.file_path}</span>
                    </Button>
                  ))}
                </div>
              )}
            </EmptyContent>
          </Empty>
        ) : (
          <div className="flex-1 overflow-y-auto px-[10vw] py-10">
            <Accordion type="multiple" defaultValue={["item-1"]}>
              <AccordionItem value="item-1">
                <AccordionTrigger>1. Create a nextbom database file</AccordionTrigger>
                <AccordionContent forceMount>
                  <CreateNextbomFile />
                </AccordionContent>
              </AccordionItem>
            </Accordion>
          </div>
        )}
      </div>
      <div className="w-full flex items-center px-3 gap-4 bg-card border-t min-h-[24px] text-xs text-muted-foreground select-none">
        {project && (
          <>
            <span className="font-medium text-foreground">{project.title ?? "Untitled"}</span>
            {project.engineer && <span>{project.engineer}</span>}
            <span className="ml-auto">Modified: {formatLastChange(project.last_change)}</span>
          </>
        )}
      </div>
    </div>
  );
}

function CreateNextbomFile(){
  const { project } = useProjectStore();

  const [csvLoaded, setCsvLoaded] = useState(false);
  const [fileCreated, setFileCreated] = useState(false);

  const [pcbNameAuto, setPcbNameAuto] = useState(true);
  const [pcbNameManual, setPcbNameManual] = useState("");

  const [version, setVersion] = useState("");
  const [versionError, setVersionError] = useState(false);

  const pcbName = pcbNameAuto ? (project?.title ?? "") : pcbNameManual;

  const handleImportCsv = async () => {
    try {
      const { count } = await invoke<{ count: number; filename_stem: string }>("load_csv");
      setCsvLoaded(true);
      toast.success(`Loaded ${count} entries from CSV`);
    } catch (error: any) {
      if (error !== "No file selected") {
        toast.error(error.toString());
      }
    }
  };

  const handleCreateFile = async () => {
    if (!/^\d+$/.test(version)) {
      setVersionError(true);
      return;
    }
    setVersionError(false);
    try {
      const result = await invoke<string>("create_nextbom_file", {
        pcbName,
        version,
      });
      setFileCreated(true);
      toast.success(result);
    } catch (error: any) {
      if (error !== "No save location selected") {
        toast.error(error.toString());
      }
    }
  };

  return (
    <div className="flex flex-col items-start gap-6 ml-5">
      <Field>
        <FieldLabel>Select CSV file</FieldLabel>
        <div className="flex flex-row items-center">
          <Button onClick={handleImportCsv}>Import CSV</Button>
          {csvLoaded && <Check className="ml-2 size-4 text-green-500" />}
          <Popover>
            <PopoverTrigger><Button variant={"ghost"} size={"icon-sm"} className="rounded-full ml-2"><Info /></Button></PopoverTrigger>
            <PopoverContent className="select-none">
              <p>
                CSV format:<br />
                <code>part ID;Designator</code><br />
                (semicolon-separated)
              </p>
            </PopoverContent>
          </Popover>
        </div>
      </Field>

      <div className="flex flex-row flex-wrap gap-6">
        <Field className="min-w-60">
          <FieldLabel>PCBA name</FieldLabel>
          <div className="flex flex-row items-center">
            <Input
              value={pcbName}
              disabled={pcbNameAuto}
              onChange={(e) => setPcbNameManual(e.target.value)}
            />
            <div className="flex items-center gap-2 ml-4">
              <Checkbox checked={pcbNameAuto} onCheckedChange={(v) => setPcbNameAuto(!!v)} />
              <p>Auto</p>
            </div>
          </div>
        </Field>

        <Field>
          <FieldLabel>BOM version</FieldLabel>
          <div className="flex flex-row items-center">
            <Input
              className="w-20"
              placeholder="1"
              value={version}
              onChange={(e) => { setVersion(e.target.value); setVersionError(false); }}
            />
            <Popover>
              <PopoverTrigger><Button variant={"ghost"} size={"icon-sm"} className="rounded-full ml-2"><Info /></Button></PopoverTrigger>
              <PopoverContent className="select-none">Increase the BOM version when the design evolves over time. This field is not meant to be used to identify design variants.</PopoverContent>
            </Popover>
          </div>
          <FieldError className={versionError ? "" : "invisible"}>BOM version must be a number</FieldError>
        </Field>
      </div>

      <div className="flex flex-row items-center">
        <Button onClick={handleCreateFile} disabled={!csvLoaded}>Create NextBOM working file</Button>
        {fileCreated && <Check className="ml-2 size-4 text-green-500" />}
      </div>
    </div>
  )
}

export default App;
