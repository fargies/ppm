// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { ref } from "vue";
import { DateTime } from "luxon";
import { useInterval } from "@vueuse/core";

export const servicesInfo = ref(null);
export const servicesStats = ref({});
export const serviceNameMap = ref({});
export const errors = ref([]);
export const now = ref(DateTime.now());

useInterval(1000, {
  callback() {
    now.value = DateTime.now();
  },
});
