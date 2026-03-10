import { execSync } from "node:child_process";
import * as path from "node:path";
import { defineConfig } from "rspress/config";

const gitHash = execSync("git describe --tags HEAD").toString().trim();

export default defineConfig({
  root: path.join(__dirname, "docs"),
  title: "PPM",
  icon: "/logo.svg",
  base: "/ppm/",
  logo: {
    light: "/logo.svg",
    dark: "/logo.svg",
  },
  themeConfig: {
    enableContentAnimation: true,
    enableAppearanceAnimation: true,
    socialLinks: [
      {
        icon: "github",
        mode: "link",
        content: "https://github.com/fargies/ppm",
      },
    ],
    footer: {
      message:
        `<a class="x-link" href="https://github.com/fargies/ppm">ppm @ ${gitHash}</a> by <a class="x-link" href="https://github.com/fargies">Sylvain Fariger</a>` +
        '<div class="footer-sponsor-links">' +
        '<a href="https://github.com/sponsors/fargies" target="_blank" rel="noopener noreferrer"><img src="https://img.shields.io/badge/Sponsor-GitHub-ea4aaa?logo=githubsponsors" alt="GitHub Sponsors" class="medium-zoom-image"></a>' +
        '<a href="https://www.buymeacoffee.com/fargies" target="_blank" rel="noopener noreferrer"><img src="https://img.shields.io/badge/Buy%20Me%20A%20Coffee-FFDD00?logo=buymeacoffee&amp;logoColor=black" alt="Buy Me A Coffee" class="medium-zoom-image"></a>' +
        "</div>",
    },
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
