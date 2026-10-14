<script setup>
import { ChevronDown } from '@lucide/vue';
</script>
<style scoped>

</style>
<template>
    <details class="group border-border border-b duration-5000 transition-all">
        <summary
            class="focus-visible:ring-2 ring-ring p-1 flex justify-between w-full cursor-pointer list-none items-center text-left font-medium outline-none transition-colors hover:text-muted-foreground focus-visible:accent"
        >
            <slot name="summary" />
            <div class="justify-end">
                <ChevronDown
                    class="size-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-out group-open:rotate-180"
                />
            </div>
        </summary>
        <div class="group-open:fx-appear content px-1 pb-2"><slot /></div>
    </details>
</template>
