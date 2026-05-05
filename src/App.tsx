import "./App.css";
import { useEffect, useState } from "react";
import { loadSavedTheme } from "./lib/custom-theme";
import { Check, ListPlus } from "lucide-react";
import { Titlebar } from "./components/titlebar";
import { Button } from "./components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "./components/ui/empty";
import { Input } from "./components/ui/input";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "./components/ui/accordion";
import { Tooltip, TooltipContent, TooltipTrigger } from "./components/ui/tooltip";
import { Field, FieldError, FieldLabel } from "./components/ui/field";
import { Checkbox } from "./components/ui/checkbox";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./components/ui/select";
import { NewProjectDialog } from "./components/new-project";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { useProjectStore } from "./store/project-store";
import { formatLastChange } from "./lib/utils";

function App() {
  const { project, recentProjects, initialize } = useProjectStore();
  const [pendingNextbomPath, setPendingNextbomPath] = useState("");
  const [pendingResolvedPath, setPendingResolvedPath] = useState("");

  useEffect(() => {
    loadSavedTheme();
    initialize();
  }, []);

  // Reset cross-step "use pending" paths whenever the active project changes.
  useEffect(() => {
    setPendingNextbomPath("");
    setPendingResolvedPath("");
  }, [project?.uuid]);

  return (
    <div className="h-screen flex flex-col overflow-clip">
      <Titlebar/>
      <div className="flex flex-col flex-1 min-h-0">
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
                <AccordionTrigger>1. Create a NextBOM file</AccordionTrigger>
                <AccordionContent>
                  <CreateNextbomFile key={project.uuid} onFileCreated={setPendingNextbomPath} />
                </AccordionContent>
              </AccordionItem>
              <AccordionItem value="item-2">
                <AccordionTrigger>2. Resolve Manufacturers &amp; MPNs</AccordionTrigger>
                <AccordionContent>
                  <ResolveManufacturers key={project.uuid} pendingNextbomPath={pendingNextbomPath} onResolved={setPendingResolvedPath} />
                </AccordionContent>
              </AccordionItem>
              <AccordionItem value="item-3">
                <AccordionTrigger>3. Generate BOM Output</AccordionTrigger>
                <AccordionContent>
                  <ExportBom key={project.uuid} pendingResolvedPath={pendingResolvedPath} />
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

function CreateNextbomFile({ onFileCreated }: { onFileCreated?: (path: string) => void }) {
  const { project } = useProjectStore();
  const [csvLoaded, setCsvLoaded] = useState(false);
  const [csvPath, setCsvPath] = useState("");
  const [fileCreated, setFileCreated] = useState(false);
  const [nextbomPath, setNextbomPath] = useState("");
  const [pcbNameAuto, setPcbNameAuto] = useState(true);
  const [pcbNameAutoSource, setPcbNameAutoSource] = useState<"project" | "csv">("project");
  const [pcbNameManual, setPcbNameManual] = useState("");
  const [csvFilenameStem, setCsvFilenameStem] = useState("");
  const [pcbNameError, setPcbNameError] = useState(false);

  const [version, setVersion] = useState("");
  const [versionError, setVersionError] = useState(false);

  const [designVariant, setDesignVariant] = useState(project?.design_variant ?? "");

  const autoPcbName = pcbNameAutoSource === "csv" ? csvFilenameStem : (project?.title ?? "");
  const pcbName = pcbNameAuto ? autoPcbName : pcbNameManual;

  const handleImportCsv = async () => {
    try {
      const { message, csv_path, filename_stem } = await invoke<{ message: string; filename_stem: string; csv_path: string }>("load_csv");
      setCsvLoaded(true);
      setCsvPath(csv_path);
      setCsvFilenameStem(filename_stem);
      toast.success(message);
    } catch (error: any) {
      if (error !== "No file selected") {
        toast.error(error.toString());
      }
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
    try {
      const { message, nextbom_path } = await invoke<{ message: string; nextbom_path: string }>("create_nextbom_file", {
        pcbName,
        bomVersion: version,
        designVariant,
      });
      setFileCreated(true);
      setNextbomPath(nextbom_path);
      onFileCreated?.(nextbom_path);
      toast.success(message);
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
          <Tooltip>
            <TooltipTrigger asChild><Button onClick={handleImportCsv}>Import CSV file</Button></TooltipTrigger>
            <TooltipContent className="select-none">
              <p>
                CSV format:<br />
                <code>part ID;Designator</code><br />
                (semicolon-separated)
              </p>
            </TooltipContent>
          </Tooltip>
          {csvLoaded && <Check className="ml-2 size-4 text-green-500" />}
          {csvPath && <span className="ml-3 text-xs text-muted-foreground font-mono">{csvPath}</span>}
        </div>
      </Field>

      <div className="flex flex-col gap-6">
        <Field>
          <FieldLabel>PCBA name</FieldLabel>
          <div className="flex flex-col">
            <div className="flex flex-row items-center">
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="block w-72">
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
                  <TooltipContent className="select-none">Auto-fill the PCBA name from the selected source.</TooltipContent>
                </Tooltip>
                <p>Auto</p>
                <Select
                  value={pcbNameAutoSource}
                  onValueChange={(v) => setPcbNameAutoSource(v as "project" | "csv")}
                  disabled={!pcbNameAuto}
                >
                  <SelectTrigger size="sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="project">Project title</SelectItem>
                    <SelectItem value="csv" disabled={!csvFilenameStem}>CSV filename</SelectItem>
                  </SelectContent>
                </Select>
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
              <TooltipContent className="select-none">Increase the BOM version when the design<br />evolves over time. Not meant to identify<br />design variants or versions.</TooltipContent>
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
        <Button onClick={handleCreateFile} disabled={!csvLoaded}>Create NextBOM file</Button>
        {fileCreated && <Check className="ml-2 size-4 text-green-500" />}
        {nextbomPath && <span className="ml-3 text-xs text-muted-foreground font-mono">{nextbomPath}</span>}
      </div>
    </div>
  )
}

type ResolvedBomEntry = {
  designator: string;
  part_id: string;
  mfr: string[];
  mpn: string[];
  alt_mfr: string[];
  alt_mpn: string[];
};

function ResolveManufacturers({ pendingNextbomPath, onResolved }: { pendingNextbomPath?: string; onResolved?: (path: string) => void }) {
  const { project } = useProjectStore();
  const [resolved, setResolved] = useState(false);
  const [nextbomPath, setNextbomPath] = useState("");
  const [dbVersion, setDbVersion] = useState<string | null>(null);
  const [autoLoad, setAutoLoad] = useState(true);
  const [bomEntries, setBomEntries] = useState<ResolvedBomEntry[]>([]);

  useEffect(() => {
    if (!project?.database_path) return;
    invoke<{ database_version: string | null; available_alternatives: string[] }>("get_database_info")
      .then(({ database_version }) => setDbVersion(database_version))
      .catch(() => {});
  }, [project?.database_path]);

  const handleResolve = async () => {
    try {
      const { message, nextbom_path } = await invoke<{ message: string; nextbom_path: string }>("resolve_bom_manufacturers", { usePending: autoLoad && !!pendingNextbomPath });
      setResolved(true);
      setNextbomPath(nextbom_path);
      onResolved?.(nextbom_path);
      toast.success(message);
      const entries = await invoke<ResolvedBomEntry[]>("get_resolved_bom", { nextbomPath: nextbom_path });
      setBomEntries(entries);
    } catch (error: any) {
      if (error !== "No file selected") {
        toast.error(error.toString());
      }
    }
  };

  const infoRows: [string, string][] = [
    ["database", project?.database_path ?? "—"],
    ["database version", dbVersion ?? "—"],
    ["active alternative", project?.project_specifics ?? "—"],
  ];

  const grouped = Object.values(
    bomEntries.reduce<Record<string, { part_id: string; designators: string[]; mfr: string[]; mpn: string[]; alt_mfr: string[]; alt_mpn: string[] }>>(
      (acc, e) => {
        if (acc[e.part_id]) {
          acc[e.part_id].designators.push(e.designator);
        } else {
          acc[e.part_id] = { part_id: e.part_id, designators: [e.designator], mfr: e.mfr, mpn: e.mpn, alt_mfr: e.alt_mfr, alt_mpn: e.alt_mpn };
        }
        return acc;
      },
      {}
    )
  );

  const hasAlts = grouped.some((g) => g.alt_mpn.length > 0);

  return (
    <div className="flex flex-col items-start gap-6 ml-5">
      <div className="flex items-center gap-2">
        <Checkbox
          id="auto-load"
          checked={autoLoad && !!pendingNextbomPath}
          disabled={!pendingNextbomPath}
          onCheckedChange={(v) => setAutoLoad(!!v)}
        />
        <label htmlFor="auto-load" className={`text-sm select-none ${!pendingNextbomPath ? "text-muted-foreground/50" : "cursor-pointer"}`}>
          Load NextBOM from step 1
        </label>
      </div>
      <div className="flex flex-col gap-1 text-xs font-mono">
        {infoRows.map(([label, value]) => (
          <div key={label} className="flex gap-2">
            <span className="text-muted-foreground/60 w-40 shrink-0">{label}</span>
            <span className="text-muted-foreground">{value}</span>
          </div>
        ))}
      </div>
      <div className="flex flex-row items-center">
        <Button onClick={handleResolve}>Resolve</Button>
        {resolved && <Check className="ml-2 size-4 text-green-500" />}
        {nextbomPath && <span className="ml-3 text-xs text-muted-foreground font-mono">{nextbomPath}</span>}
      </div>
      {grouped.length > 0 && (
        <div className="w-full overflow-auto max-h-72 rounded border border-border">
          <table className="w-full text-xs font-mono border-collapse">
            <thead className="sticky top-0 bg-background border-b border-border">
              <tr>
                <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">Part ID</th>
                <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">Designators</th>
                <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">Manufacturer</th>
                <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">MPN</th>
                {hasAlts && <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">Alternative Manufacturer</th>}
                {hasAlts && <th className="text-left px-3 py-1.5 text-muted-foreground font-medium">Alternative MPN</th>}
              </tr>
            </thead>
            <tbody>
              {grouped.map((g) => (
                <tr key={g.part_id} className="border-t border-border/50 hover:bg-muted/30">
                  <td className="px-3 py-1">{g.part_id}</td>
                  <td className="px-3 py-1 text-muted-foreground">{g.designators.join(", ")}</td>
                  <td className="px-3 py-1">
                    {g.mfr.length > 0 ? g.mfr.map((m) => <div key={m}>{m}</div>) : <span className="text-muted-foreground/40">—</span>}
                  </td>
                  <td className="px-3 py-1">
                    {g.mpn.length > 0 ? g.mpn.map((m) => <div key={m}>{m}</div>) : <span className="text-muted-foreground/40">—</span>}
                  </td>
                  {hasAlts && (
                    <td className="px-3 py-1">
                      {g.alt_mfr.length > 0 ? g.alt_mfr.map((m) => <div key={m}>{m}</div>) : <span className="text-muted-foreground/40">—</span>}
                    </td>
                  )}
                  {hasAlts && (
                    <td className="px-3 py-1 text-muted-foreground">
                      {g.alt_mpn.length > 0 ? g.alt_mpn.map((m) => <div key={m}>{m}</div>) : <span className="text-muted-foreground/40">—</span>}
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function ExportBom({ pendingResolvedPath }: { pendingResolvedPath?: string }) {
  const { project } = useProjectStore();
  const [autoLoad, setAutoLoad] = useState(true);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleExport = async () => {
    setLoading(true);
    try {
      const path = await invoke<string>("export_bom_to_excel", {
        nextbomPath: autoLoad && pendingResolvedPath ? pendingResolvedPath : null,
      });
      setOutputPath(path);
      toast.success("BOM exported successfully");
    } catch (error: any) {
      if (error !== "cancelled") {
        toast.error(error.toString());
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex flex-col items-start gap-6 ml-5">
      <div className="flex items-center gap-2">
        <Checkbox
          id="auto-load-3"
          checked={autoLoad && !!pendingResolvedPath}
          disabled={!pendingResolvedPath}
          onCheckedChange={(v) => setAutoLoad(!!v)}
        />
        <label htmlFor="auto-load-3" className={`text-sm select-none ${!pendingResolvedPath ? "text-muted-foreground/50" : "cursor-pointer"}`}>
          Load resolved NextBOM from step 2
        </label>
      </div>
      <div className="flex flex-col gap-1 text-xs font-mono">
        <div className="flex gap-2">
          <span className="text-muted-foreground/60 w-40 shrink-0">BOM template</span>
          <span className="text-muted-foreground">{project?.bom_template_path ?? "—"}</span>
        </div>
      </div>
      <div className="flex flex-row items-center">
        <Button onClick={handleExport} disabled={loading}>Export to Excel</Button>
        {outputPath && <Check className="ml-2 size-4 text-green-500" />}
        {outputPath && <span className="ml-3 text-xs text-muted-foreground font-mono">{outputPath}</span>}
        {outputPath && <Button variant="outline" className="ml-3" onClick={() => openPath(outputPath).catch((e: any) => toast.error(e.toString()))}>Open file</Button>}
      </div>
    </div>
  );
}

export default App;
