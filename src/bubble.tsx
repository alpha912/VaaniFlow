// src/bubble.tsx — Floating bubble window entry point
import React from "react";
import ReactDOM from "react-dom/client";
import BubbleApp from "./BubbleApp";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BubbleApp />
  </React.StrictMode>
);
