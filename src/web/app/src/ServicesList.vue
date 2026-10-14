<script setup>
// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { ref } from "vue";
import { useInterval } from "@vueuse/core";
import WSState from "./components/WSState.vue";
import Card from "./components/Card.vue";
import Badge from "./components/Badge.vue";
import {
    CheckSquare,
    RotateCcw,
    Skull,
    SquareX,
} from "@lucide/vue";
import { servicesInfo, serviceNameMap, now } from "./store";
import { client } from "./Client";
import { formatDate, upTime as utilsUpTime, mapStatusBadge } from "./utils";

const updates = ref(0);
const uptime_width = ref(0);

client.onMessage = (_data) => {
    updates.value += 1;
};

client.request_info(null);
const interval = useInterval(5000, {
    callback: () => client.request_info(null),
});

function upTime(start_time, end_time) {
    let ret = utilsUpTime(start_time, end_time);

    if (ret?.length >= uptime_width.value) {
        uptime_width.value = ret.length + 1;
    }
    return ret;
}
</script>
<template>
    <Card class="col-span-12">
        <template #header>
            <h3 class="text-accent font-semibold">PPM Service Info</h3>
            <WSState class="ml-auto" :status="updates" />
        </template>
        <div>
            <div class="w-full px-3 table table-auto text-sm">
                <div class="table-header-group">
                    <div class="table-row">
                        <div class="table-cell">Id</div>
                        <div class="table-cell">Name</div>
                        <div class="hidden sm:table-cell">Pid</div>
                        <div class="table-cell">Active</div>
                        <div class="table-cell">Status</div>
                        <div class="sm:table-cell hidden">Start time</div>
                        <div class="table-cell">
                            Uptime
                        </div>
                        <div class="table-cell">
                            <RotateCcw aria-label="Restarts" />
                        </div>
                        <div class="table-cell">
                            <Skull arial-label="Crashes" />
                        </div>
                    </div>
                </div>
                <div
                    :class="{ skel: !servicesInfo }"
                    class="table-row-group skel-anim"
                >
                    <div
                        v-for="(info, id) in servicesInfo || {
                            0: null,
                            1: null,
                            2: null,
                        }"
                        :key="id"
                        :class="{ 'fx-appear': servicesInfo }"
                        class="table-row text-center hover:bg-foreground/5 cursor-pointer"
                        @click="
                            $router.push({
                                name: 'service',
                                params: { id },
                            })
                        "
                    >
                        <div class="table-cell font-mono">{{ id }}</div>
                        <div class="table-cell font-mono">
                            {{ serviceNameMap?.[id] ?? "?" }}
                        </div>
                        <div class="hidden sm:table-cell font-mono">{{ info?.pid }}</div>
                        <div class="table-cell">
                            <CheckSquare
                                class="text-success"
                                v-if="info?.active"
                            />
                            <SquareX v-else class="text-failure" />
                        </div>
                        <div class="table-cell">
                            <Badge :class="mapStatusBadge(info?.status)">{{
                                info?.status
                            }}</Badge>
                        </div>
                        <div class="sm:table-cell hidden">
                            {{ formatDate(info?.start_time) ?? "?" }}
                        </div>
                        <div
                            class="table-cell"
                            :style="`min-width: ${uptime_width}ch`"
                        >
                            {{
                                upTime(
                                    info?.start_time,
                                    info?.end_time ?? now,
                                ) ?? "?"
                            }}
                        </div>
                        <div class="table-cell">{{ info?.restarts }}</div>
                        <div class="table-cell">{{ info?.crashed }}</div>
                    </div>
                </div>
            </div>
        </div>
    </Card>
</template>
