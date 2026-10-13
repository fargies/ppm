<script setup>
// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { defineComponent, defineProps, ref, h, computed } from "vue";
import { useInterval } from "@vueuse/core";
import {
    ChevronDown,
    CheckSquare,
    SquareX,
    Pause,
    RotateCcw,
    Square,
    Play,
} from "@lucide/vue";
import { client } from "./Client";
import { servicesInfo, serviceNameMap, now, servicesStats } from "./store";
import {
    Card,
    WSState,
    ServiceInfoPart,
    Badge,
    Button,
    Collapsible,
} from "./components";
import { mapStatusBadge, upTime, formatDuration, formatBytes } from "./utils";

let { id } = defineProps({ id: String });

const updates = ref(0);
const info = computed(() => servicesInfo.value?.[id]);
const stats = computed(() => servicesStats.value?.[id]);

client.onMessage = (data) => {
    updates.value += 1;
};
client.request_info(id);
const interval = useInterval(5000, {
    callback: () => {
        client.request_info(id);
        client.request_stats(id);
    },
});
</script>

<template>
    <Card>
        <template #header>
            <h3
                :class="{ skel: !serviceNameMap?.[id] }"
                class="skel-anim font-semibold text-accent"
            >
                {{ serviceNameMap?.[id] ?? "placeholder" }}
            </h3>
            <WSState class="ml-auto" :status="updates" />
        </template>
        <Collapsible open>
            <template #summary> Controls </template>
            <div class="flex gap-1 justify-center md:justify-start md:ps-2">
                <Button
                    :skel="!info"
                    :disabled="!info?.pid"
                    label="Stop"
                    @click="
                        client.send({
                            method: 'stop',
                            params: { service: id },
                        })
                    "
                >
                    <Square />Stop
                </Button>
                <Button
                    :skel="!info"
                    label="Run"
                    @click="
                        client.send({
                            method: 'restart',
                            params: { service: id },
                        })
                    "
                >
                    <template v-if="info?.pid"><RotateCcw /> Restart</template>
                    <template v-else> <Play /> Start</template>
                </Button>
            </div>
        </Collapsible>
        <Collapsible open>
            <template #summary> Status </template>
            <div
                class="grid grid-cols-1 sm:grid-cols-3 lg:grid-cols-6 gap-1 p-1"
            >
                <ServiceInfoPart :skel="!info" title="Id">{{
                    id
                }}</ServiceInfoPart>
                <ServiceInfoPart
                    :skel="!info"
                    class="sm:col-span-2 lg:col-span-5"
                    title="Name"
                    >{{ serviceNameMap?.[id] || "?" }}</ServiceInfoPart
                >
                <ServiceInfoPart :skel="!info" title="Status"
                    ><Badge :class="mapStatusBadge(info?.status)">{{
                        info?.status ?? "?"
                    }}</Badge></ServiceInfoPart
                >
                <ServiceInfoPart :skel="!info" title="Active"
                    >{{ info?.active ?? "?" }}
                </ServiceInfoPart>
                <ServiceInfoPart :skel="!info" title="Pid">{{
                    info?.pid ?? "?"
                }}</ServiceInfoPart>
                <ServiceInfoPart :skel="!info" title="Uptime">{{
                    upTime(info?.start_time, info?.end_time ?? now) ?? "?"
                }}</ServiceInfoPart>
                <ServiceInfoPart :skel="!info" title="Restarts">{{
                    info?.restarts ?? "?"
                }}</ServiceInfoPart>
                <ServiceInfoPart :skel="!info" title="Crashes">{{
                    info?.crashed ?? "?"
                }}</ServiceInfoPart>
            </div>
        </Collapsible>
        <Collapsible v-show="info?.pid">
            <template #summary> Statistics </template>
            <div
                class="grid grid-cols-1 sm:grid-cols-3 lg:grid-cols-6 gap-1 p-1"
            >
                <ServiceInfoPart :skel="!stats" title="CPU"
                    >{{ stats?.cpu_usage?.toFixed(2) }}%</ServiceInfoPart
                >
                <ServiceInfoPart :skel="!stats" title="Cpu Time">{{
                    formatDuration(stats?.cpu_time) ?? "?"
                }}</ServiceInfoPart>
                <ServiceInfoPart :skel="!stats" title="I/O read">
                    {{ formatBytes(stats?.io_read) }}/sec
                </ServiceInfoPart>
                <ServiceInfoPart :skel="!stats" title="I/O write"
                    >{{ formatBytes(stats?.io_write) }}/sec
                </ServiceInfoPart>
                <ServiceInfoPart :skel="!stats" title="I/O read">{{
                    formatBytes(stats?.total_io_read) ?? "?"
                }}</ServiceInfoPart>
                <ServiceInfoPart :skel="!stats" title="I/O write">{{
                    formatBytes(stats?.total_io_write) ?? "?"
                }}</ServiceInfoPart>
                <ServiceInfoPart :skel="!stats" title="Mem RSS">{{
                    formatBytes(stats?.mem_rss) ?? "?"
                }}</ServiceInfoPart>
                <ServiceInfoPart :skel="!stats" title="Mem VSZ">{{
                    formatBytes(stats?.mem_vsz) ?? "?"
                }}</ServiceInfoPart>
            </div>
        </Collapsible>
        <div></div>
    </Card>
</template>
