import "./App.css";
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Info, ListPlus } from "lucide-react";
import { Titlebar } from "./components/titlebar";
import { Button } from "./components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "./components/ui/empty";
import { Input } from "./components/ui/input";
import { Label } from "./components/ui/label";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "./components/ui/accordion";
import { Popover, PopoverContent, PopoverTrigger } from "./components/ui/popover";
import { Field, FieldDescription, FieldError, FieldLabel } from "./components/ui/field";
import { Checkbox } from "./components/ui/checkbox";
import { Project } from "./types/Project";
import { NewProjectDialog } from "./components/new-project";
import { ProjectState } from "./types/ProjectState";

function App() {
  const [currentProject, setCurrentProject] = useState<Project | null>(null);

  useEffect(() => {
    // Listen for project changes from backend
    listen<ProjectState>("project-changed", (event) => {
      setCurrentProject(event.payload.project);
    });
  }, []);

  return (
    <div className="h-screen flex flex-col overflow-clip">
      <Titlebar/>
      <div className="flex flex-col flex-1 overflow-clip">
        {!currentProject ? (
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
                {/* <Button variant="outline">Open Project</Button> */}
              </div>
              {/* <div className="flex flex-col items-start min-w-60 w-[30vw] pt-2">
                <p className="text-lg font-medium pb-2">Recent</p>
                <div className="flex flex-row">
                  <a href="#" className="text-start pr-10 text-primary font-medium">filename</a>
                  <div className="text-start text-muted-foreground">6:03:47 PM [vite] (client) hmr update /src/App.tsx, /src/App.css6:03:47 PM [vite] (client) hmr update /src/App.tsx, /src/App.css6:03:47</div>
                </div>
              </div> */}
            </EmptyContent>
          </Empty>
        ) : (
          <div className="flex-1 overflow-y-auto px-[10vw] py-10">
            <Accordion type="multiple" defaultValue={["item-1"]}>
              <AccordionItem value="item-1">
                <AccordionTrigger>1. Create a nextbom database file</AccordionTrigger>
                <AccordionContent>
                  <CreateNextbomFile />
                </AccordionContent>
              </AccordionItem>

              <AccordionItem value="item-2">
                <AccordionTrigger>2. Add part numbers</AccordionTrigger>
                <AccordionContent>
                  <AddPartNumbers />
                </AccordionContent>
              </AccordionItem>
            </Accordion>
          </div>
        )}
      </div>
      <div className="w-full flex items-center justify-center bg-primary min-h-[24px]">
      </div>
    </div>
  );
}

function CreateNextbomFile(){
  return (
    <div className="flex flex-col items-start gap-6 ml-5">
      <Field>
        <FieldLabel>Select CSV file</FieldLabel>
        <div className="flex flex-row items-center">
          <Input type="file" />
          <Popover>
            <PopoverTrigger><Button variant={"ghost"} size={"icon-sm"} className="rounded-full ml-2"><Info /></Button></PopoverTrigger>
            <PopoverContent className="select-none">csv file explanation</PopoverContent>
          </Popover>
        </div>
      </Field>

      <div className="flex flex-row justify-between w-full gap-10">
        <Field className="min-w-60">
          <FieldLabel>Set PCBA name</FieldLabel>
          <div className="flex flex-row items-center">
            <Input />
            <div className="flex items-center gap-2 ml-4">
              <Checkbox defaultChecked={true}/>
              <p>Auto</p>
            </div>
          </div>
        </Field>
        
        <Field className="min-w-60 w-auto">
          <FieldLabel>Set BOM version</FieldLabel>
          <div className="flex flex-row items-center">
            <Input className="w-20"/>
            <div className="flex items-center gap-2 ml-4">
              <Checkbox defaultChecked={true}/>
              <p>Auto</p>
            </div>
            <Popover>
              <PopoverTrigger><Button variant={"ghost"} size={"icon-sm"} className="rounded-full ml-2"><Info /></Button></PopoverTrigger>
              <PopoverContent className="select-none">Increase the BOM version when the design evolves over time. This field is not meant to be used to identify design variants.</PopoverContent>
            </Popover>
          </div>
          <FieldError>BOM version must be a number</FieldError>
        </Field>

      </div>

      
      
      

      <p></p>
      <p>auto (increment from project settings)</p>

      
    </div>
  )
}

function AddPartNumbers(){
  return (
    <div className="grid w-full items-center gap-3">
      test
    </div>
  )
}

export default App;
