<script setup>
import { ref, useTemplateRef, watch, defineProps } from "vue";
import { ChevronDown } from "@lucide/vue";

const { open } = defineProps({
    open: {
        type: Boolean,
        default: false,
    },
});

const state = ref(open);
watch(
    () => open,
    (value) => {
        state.value = value;
    },
);

function toggle() {
  state.value = !state.value
}
</script>
<template>
    <details
        :open="state"
        class="group border-border border-b duration-5000 transition-all"
    >
        <summary
            @click.prevent="toggle"
            class="focus-visible:ring-2 ring-ring p-1 flex justify-between w-full cursor-pointer list-none items-center text-left font-medium outline-none transition-colors hover:text-muted-foreground focus-visible:accent"
        >
            <slot name="summary" />
            <div class="justify-end">
                <ChevronDown
                    class="size-4 shrink-0 text-muted-foreground transition-transform duration-200 ease-out group-open:rotate-180"
                />
            </div>
        </summary>
        <div class="group-open:fx-appear content px-1 pb-2">
            <slot :open="state" />
        </div>
    </details>
</template>
