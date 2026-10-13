import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import vueDevTools from "vite-plugin-vue-devtools";

// https://vite.dev/config/
export default defineConfig(({ command, mode }) => {
  if (command == "build") {
    return { plugins: [vue(), tailwindcss()] };
  } else {
    return {
      define: { __API_URL__: "\"ws://localhost:3000/ws\"" },
      plugins: [vue(), tailwindcss(), vueDevTools()],
      build: { sourcemap: true },
    };
  }
});
