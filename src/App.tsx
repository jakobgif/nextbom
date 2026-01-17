import "./App.css";
import { ListPlus } from "lucide-react";
import { Titlebar } from "./components/titlebar";
import { Button } from "./components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "./components/ui/empty";

function App() {
  return (
    <div className="h-screen flex flex-col">
      <Titlebar/>
      {true ? (
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
              <Button>Create Project</Button>
              <Button variant="outline">Open Project</Button>
            </div>
            <div className="flex flex-col items-start min-w-60 w-[30vw] pt-2">
              <p className="text-lg font-medium pb-2">Recent</p>
              <div className="flex flex-row">
                <p className="text-start pr-10">filename</p>
                <div className="text-start text-muted-foreground">6:03:47 PM [vite] (client) hmr update /src/App.tsx, /src/App.css6:03:47 PM [vite] (client) hmr update /src/App.tsx, /src/App.css6:03:47</div>
              </div>
            </div>
          </EmptyContent>
        </Empty>
      ) : (
        <div></div>
      )}
    </div>
  );
}

export default App;
