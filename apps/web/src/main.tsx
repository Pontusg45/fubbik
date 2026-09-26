import "./index.css";
import { RouterProvider } from "@tanstack/react-router";
import { createRoot } from "react-dom/client";

import { getRouter } from "./router";

const root = document.getElementById("root");
if (!root) throw new Error("Missing root element");

createRoot(root).render(<RouterProvider router={getRouter()} />);
