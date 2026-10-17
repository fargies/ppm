<script setup>
import { ref, defineProps, defineEmits, watch } from "vue";
import { useWebSocket } from "@vueuse/core";
import { api_url } from "../Client";
import { addError } from "../store";

const logLines = ref(null);
const { id, lines, follow, max_lines } = defineProps({
    id: { type: String },
    lines: { type: Number },
    follow: { type: Boolean, default: false },
    max_lines: { type: Number },
});
const emit = defineEmits(["update"]);

function logStream() {
    let { close, send } = useWebSocket(api_url + "?raw=1", {
        autoReconnect: { delay: 1000 },
        onError(err) {
            addError(err?.message ?? "server connection closed");
        },
        onMessage: async (_, event) => {
            let new_lines = (await event.data.text()).split("\n");
            if (logLines.value === null) {
                logLines.value = [];
            }
            logLines.value.push(...new_lines);
            if (logLines.value.length > max_lines) {
                logLines.value.splice(0, logLines.value.length - max_lines);
            }
            emit("update");
        },
        onDisconnected() {
            logLines.value = null;
        },
    });
    send(
        JSON.stringify({
            method: "log",
            params: { service: id, lines, follow },
        }),
    );
    return close;
}
let close = logStream();
watch(
    () => id,
    (_) => {
        close?.();
        close = logStream();
    },
);
</script>
<style scoped>
pre {
    text-indent: 3em hanging each-line;
    text-wrap: wrap;
    background-clip: content-box;
}
</style>
<template>
    <div class="p-1 w-full overflow-y-auto">
        <pre
            :key="idx"
            v-for="(line, idx) in logLines ?? Array(max_lines ?? 3).fill(null)"
            class="py-0.5 font-mono text-xs skel-anim"
            :class="{
                skel: line === null,
                'w-6/12': line === null && idx % 3 === 0,
                'w-5/12': line === null && idx % 3 === 1,
                'w-8/12': line === null && idx % 3 === 2,
            }"
            >{{ (line ?? "placeholder") || " " }}</pre>
    </div>
</template>
