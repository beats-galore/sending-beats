
import { createRoot } from "react-dom/client";
import { MantineProvider } from '@mantine/core';
import App from "./App";
import "./styles.css";
import '@mantine/core/styles.css';

const container = document.getElementById("root");
if (container) {
  const root = createRoot(container);
  root.render(
    <MantineProvider>
      <App />
    </MantineProvider>
  );
} 