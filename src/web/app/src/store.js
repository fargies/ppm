// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { ref, watch } from "vue";
import { DateTime } from "luxon";
import { useInterval } from "@vueuse/core";

const ERRORS_MAX = 10;
let error_num = 0;

export const servicesInfo = ref(null);
export const servicesStats = ref({});
export const serviceNameMap = ref({});
export const errors = ref([]);
export const now = ref(DateTime.now());

export const theme = ref(
  localStorage.theme ??
  window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
);

function setTheme(theme) {
  if (theme === "dark") {
    document.documentElement.classList.add("dark");
  } else document.documentElement.classList.remove("dark");
}

watch(theme, (theme) => {
  localStorage.theme = theme;
  setTheme(theme);
});
setTheme(theme.value);
document.getElementsByTagName('body')[0]?.classList?.add("transition-colors");

export function toggleTheme() {
  theme.value = theme.value === "dark" ? "light" : "dark";
}

export function addError(err) {
  if (errors.value.length >= ERRORS_MAX) {
    errors.value.splice(0, 1);
  }
  if (typeof err === "string") {
    err = { message: err };
  }
  if (typeof err === "object") {
    err.id = error_num++;
  }
  errors.value.push(err);
}

useInterval(1000, {
  callback() {
    now.value = DateTime.now();
  },
});
