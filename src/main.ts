import { getBackend } from "./api";
import { createApp } from "./app";
import "./styles.css";

const root = document.getElementById("app");
if (root) {
  const backend = getBackend();
  if (backend.isMock) document.title = "LoL Game Review (mode simulé)";
  createApp(root, backend);
}
