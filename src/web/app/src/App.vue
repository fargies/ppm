<script setup>
// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import logoUrl from "../../../../data/ppm.svg";
import { Moon, Sun, X } from "@lucide/vue";
import { errors, theme, toggleTheme } from "./store";
import { useRouter } from "vue-router";

import { Button } from "./components";

let router = useRouter();
let h = router.options.history;
h.state;
</script>
<template>
    <nav
        class="sticky z-100 flex w-full justify-between mb-1 px-1 shadow-sm gap-2 top-0 left-0 items-center border-b border-border bg-background"
    >
        <RouterLink class="flex gap-1 items-center" :to="{ name: 'list' }">
            <img id="logo" class="w-12 h-12" :src="logoUrl" />
            <h1 class="text-accent text-2xl font-bold">PPM</h1>
        </RouterLink>
        <Button title="theme switch" @click="toggleTheme"><Moon v-if="theme === 'dark'"/><Sun v-else /></Button>
    </nav>
    <div class="grid grid-cols-12 gap-1 col-span-12 p-1">
        <RouterView />
        <div
            v-if="errors.length > 0"
            title="application errors"
            class="fx-appear col-span-12 border-destructive border rounded shadow-sm p-1 text-destructive"
        >
            <div
                class="flex fx-appear col-span-12 items-center gap-1 hover:bg-foreground/5"
                :key="error?.id ?? idx"
                v-for="(error, idx) in errors"
            >
                <span class="font-mono text-xs text-muted-foreground"
                    >[{{ error?.id }}]</span
                >
                <span class="grow"
                    ><span class="font-bold">ERROR: </span>
                    {{ error?.message ?? error }}</span
                >
                <button class="cursor-pointer" @click="errors.splice(idx, 1)">
                    <X />
                </button>
            </div>
        </div>
    </div>
</template>
