import "./App.css";
import { useEffect, useState } from "react";
import { Check, ListPlus } from "lucide-react";
import { Titlebar } from "./components/titlebar";
import { Button } from "./components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "./components/ui/empty";
import { Input } from "./components/ui/input";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "./components/ui/accordion";
import { Tooltip, TooltipContent, TooltipTrigger } from "./components/ui/tooltip";
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
                <AccordionContent>
                  <CreateNextbomFile key={project.uuid} />
                </AccordionContent>
              </AccordionItem>
              <AccordionItem value="item-2">
                <AccordionTrigger>2. Resolve manufacturers &amp; MPNs</AccordionTrigger>
                <AccordionContent>
                  <ResolveManufacturers key={project.uuid} />
                </AccordionContent>
              </AccordionItem>
            </Accordion>
          </div>
        )}
      </div>
      <div className="w-full flex items-center px-3 gap-3 bg-card border-t min-h-[24px] text-xs text-muted-foreground select-none">
        {project && Object.entries(project).map(([key, value]) => (
          <span key={key}>
            <span className="text-muted-foreground/60">{key}: </span>
            {key === "last_change" ? formatLastChange(value as unknown as bigint) : String(value ?? "null")}
          </span>
        ))}
      </div>
    </div>
  );
}

function CreateNextbomFile(){
  const { project } = useProjectStore();
  const [csvLoaded, setCsvLoaded] = useState(false);
  const [fileCreated, setFileCreated] = useState(false);
  const [csvPicking, setCsvPicking] = useState(false);
  const [fileCreating, setFileCreating] = useState(false);

  const [pcbNameAuto, setPcbNameAuto] = useState(true);
  const [pcbNameManual, setPcbNameManual] = useState("");
  const [pcbNameError, setPcbNameError] = useState(false);

  const [version, setVersion] = useState("");
  const [versionError, setVersionError] = useState(false);

  const [designVariant, setDesignVariant] = useState(project?.design_variant ?? "");

  const pcbName = pcbNameAuto ? (project?.title ?? "") : pcbNameManual;

  const handleImportCsv = async () => {
    setCsvPicking(true);
    try {
      const { message } = await invoke<{ message: string; filename_stem: string }>("load_csv");
      setCsvLoaded(true);
      toast.success(message);
    } catch (error: any) {
      if (error !== "No file selected") {
        toast.error(error.toString());
      }
    } finally {
      setCsvPicking(false);
    }
  };

  const handleCreateFile = async () => {
    if (!pcbName.trim()) {
      setPcbNameError(true);
      return;
    }
    setPcbNameError(false);
    if (!/^\d+$/.test(version)) {
      setVersionError(true);
      return;
    }
    setVersionError(false);
    setFileCreating(true);
    try {
      const result = await invoke<string>("create_nextbom_file", {
        pcbName,
        version,
        designVariant,
      });
      setFileCreated(true);
      toast.success(result);
    } catch (error: any) {
      if (error !== "No save location selected") {
        toast.error(error.toString());
      }
    } finally {
      setFileCreating(false);
    }
  };

  return (
    <div className="flex flex-col items-start gap-6 ml-5">
      <Field>
        <FieldLabel>Select CSV file</FieldLabel>
        <div className="flex flex-row items-center">
          <Tooltip>
            <TooltipTrigger asChild><Button onClick={handleImportCsv} disabled={csvPicking}>Import CSV</Button></TooltipTrigger>
            <TooltipContent className="select-none">
              <p>
                CSV format:<br />
                <code>part ID;Designator</code><br />
                (semicolon-separated)
              </p>
            </TooltipContent>
          </Tooltip>
          {csvLoaded && <Check className="ml-2 size-4 text-green-500" />}
        </div>
      </Field>

      <div className="flex flex-col gap-6">
        <Field className="w-96">
          <FieldLabel>PCBA name</FieldLabel>
          <div className="flex flex-col">
            <div className="flex flex-row items-center">
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="block w-full">
                    <Input
                      value={pcbName}
                      disabled={pcbNameAuto}
                      onChange={(e) => { setPcbNameManual(e.target.value); setPcbNameError(false); }}
                    />
                  </span>
                </TooltipTrigger>
                <TooltipContent className="select-none">Name used in the generated BOM file.</TooltipContent>
              </Tooltip>
              <div className="flex items-center gap-2 ml-4">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span><Checkbox checked={pcbNameAuto} onCheckedChange={(v) => { setPcbNameAuto(!!v); if (v) setPcbNameError(false); }} /></span>
                  </TooltipTrigger>
                  <TooltipContent className="select-none">Use the project title as the PCBA name.</TooltipContent>
                </Tooltip>
                <p>Auto</p>
              </div>
            </div>
            {pcbNameError && <FieldError className="mt-1">PCBA name is required</FieldError>}
          </div>
        </Field>

        <Field className="w-auto">
          <FieldLabel>BOM version</FieldLabel>
          <div className="flex flex-col">
            <Tooltip>
              <TooltipTrigger asChild>
                <Input
                  className="w-20"
                  value={version}
                  onChange={(e) => { setVersion(e.target.value); setVersionError(false); }}
                />
              </TooltipTrigger>
              <TooltipContent className="select-none">Increase the BOM version when the design<br />evolves over time. Not meant to identify<br />design variants.</TooltipContent>
            </Tooltip>
            {versionError && <FieldError className="mt-1">BOM version must be a number</FieldError>}
          </div>
        </Field>

        <Field className="w-56">
          <FieldLabel>Design variant</FieldLabel>
          <Tooltip>
            <TooltipTrigger asChild>
              <Input
                placeholder="e.g. full, lite"
                value={designVariant}
                onChange={(e) => setDesignVariant(e.target.value)}
              />
            </TooltipTrigger>
            <TooltipContent className="select-none">Identifies which design variant this BOM<br />belongs to. Stored in the project for reuse.</TooltipContent>
          </Tooltip>
        </Field>
      </div>

      <div className="flex flex-row items-center">
        <Button onClick={handleCreateFile} disabled={!csvLoaded || fileCreating}>Create NextBOM working file</Button>
        {fileCreated && <Check className="ml-2 size-4 text-green-500" />}
      </div>
    </div>
  )
}

function ResolveManufacturers() {
  const [resolved, setResolved] = useState(false);
  const [resolving, setResolving] = useState(false);

  const handleResolve = async () => {
    setResolving(true);
    try {
      const result = await invoke<string>("resolve_bom_manufacturers");
      setResolved(true);
      toast.success(result);
    } catch (error: any) {
      if (error !== "No file selected") {
        toast.error(error.toString());
      }
    } finally {
      setResolving(false);
    }
  };

  return (
    <div className="flex flex-row items-center ml-5">
      <Button onClick={handleResolve} disabled={resolving}>Resolve</Button>
      {resolved && <Check className="ml-2 size-4 text-green-500" />}
    </div>
  );
}

export default App;
