import "./styles.css";
import App from "./App.svelte";
import { mount } from "svelte";

const themeStorageKey = "pdf-tools-theme";
const storedTheme = localStorage.getItem(themeStorageKey);
const initialTheme =
  storedTheme === "light" || storedTheme === "dark"
    ? storedTheme
    : window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";

document.documentElement.dataset.theme = initialTheme;

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
