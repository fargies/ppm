import * as path from "node:path";
import { defineConfig } from "rspress/config";

export default defineConfig({
  root: path.join(__dirname, "docs"),
  title: "PPM",
  icon: "/logo.svg",
  logo: {
    light: "/logo.svg",
    dark: "/logo.svg",
  },
  themeConfig: {
    socialLinks: [
      {
        icon: "github",
        mode: "link",
        content: "https://github.com/fargies/ppm",
      },
    ],
  },
  globalStyles: path.join(__dirname, "docs/index.css"),
  builderConfig: {
    html: {
      tags: [
        {
          tag: "link",
          attrs: {
            href: "https://cdn.jsdelivr.net/npm/bootstrap-icons@1.13.1/font/bootstrap-icons.min.css",
            rel: "stylesheet",
          },
        },
        {
          tag: "meta",
          attrs: {
            name: "author",
            content: "Sylvain Fargier <fargier.sylvain@gmail.com>",
          },
        },
        {
          tag: "script",
          attrs: {
            async: true,
            src: "https://www.googletagmanager.com/gtag/js?id=G-X7231F8RV9",
          },
        },
        {
          tag: "script",
          children:
            "window.dataLayer = window.dataLayer || [];\
          function gtag(){dataLayer.push(arguments);}\
          gtag('js', new Date());\
          gtag('config', 'G-X7231F8RV9');",
        },
      ],
    },
  },
});
