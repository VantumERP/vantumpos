import "./App.css";

import { AppShell } from "@/app/AppShell";
import { createLocalServices } from "@/services/local-adapter";

const services = createLocalServices();

function App() {
  return <AppShell services={services} />;
}

export default App;
