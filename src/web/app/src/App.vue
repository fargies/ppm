<script setup>
// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { X } from "@lucide/vue";

import { errors } from "./store";
</script>
<template>
    <RouterView class="col-span-12" />
    <div
        v-if="errors.length > 0"
        class="col-span-12 border-destructive border rounded shadow-sm p-1 text-destructive"
    >
        <div class="flex justify-between hover:bg-foreground/5" v-for="(error, idx) in errors">
            <span> <span class="font-bold">ERROR: </span> {{ error }}</span>
            <button class="cursor-pointer" @click="errors.splice(idx, 1)"><X /></button>
        </div>
    </div>
</template>
