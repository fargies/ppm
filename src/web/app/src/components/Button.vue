<script setup>
// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

defineProps({
    label: { type: String, default: "" },
    skel: { type: Boolean, default: false },
});
</script>
<template>
    <button
        :label="label"
        :aria-label="label"
        class="skel-anim w-fit h-fit duration-300 cursor-pointer inline-flex shrink-0 items-center justify-center border text-sm font-medium whitespace-nowrap shadow-sm hover:shadow-md transition-all disabled:cursor-default disabled:pointer-events-none disabled:opacity-50 border-border text-foreground hover:bg-foreground/5 active:bg-foreground/15 gap-1 rounded-md p-1"
        :class="{ skel }"
    >
        <slot />
    </button>
</template>
