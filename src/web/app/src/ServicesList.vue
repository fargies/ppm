<script setup>
// SPDX-License-Identifier: Zlib
// SPDX-FileCopyrightText: 2026 Sylvain Fargier
// SPDX-FileContributor: Author: Sylvain Fargier <fargier.sylvain@gmail.com>

import { DateTime, Interval, Duration } from "luxon";
import { ref } from "vue";
import { useInterval } from "@vueuse/core";
import WSState from "./components/WSState.vue";
import Card from "./components/Card.vue";
import Badge from "./components/Badge.vue";
import {
    CheckSquare,
    ChevronDown,
    RotateCcw,
    Skull,
    SquareX,
} from "@lucide/vue";
import { servicesInfo, serviceNameMap, now } from "./store";
import { client } from "./Client";
import { formatDate, upTime as utilsUpTime, mapStatusBadge } from "./utils";

const updates = ref(0);
const uptime_width = ref(0);

client.onMessage = (data) => {
    updates.value += 1;
};

// client.request_info(null);
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
    <Card>
        <template #header>
            <h3 class="font-semibold text-foreground">PPM Service Info</h3>
            <WSState class="ml-auto" :status="updates" />
        </template>
        <div>
            <table class="w-full px-3 table table-auto text-sm">
                <thead>
                    <tr>
                        <th
                            class="fx-appear text-muted-foreground"
                            v-for="name in [
                                'Id',
                                'Name',
                                'Pid',
                                'Active',
                                'Status',
                                'Start time',
                                'Uptime',
                            ]"
                        >
                            {{ name }}
                        </th>
                        <th class="text-muted-foreground align-middle">
                            <RotateCcw aria-label="Restarts" />
                        </th>
                        <th class="text-muted-foreground">
                            <Skull arial-label="Crashes" />
                        </th>
                    </tr>
                </thead>
                <tbody :class="{ skel: !servicesInfo }" class="skel-anim">
                    <tr
                        v-for="(info, id) in servicesInfo || {
                            0: null,
                            1: null,
                            2: null,
                        }"
                        :key="id"
                        :class='{ "fx-appear": servicesInfo }'
                        class="text-center hover:bg-foreground/5 cursor-pointer"
                        @click="
                            $router.push({
                                name: 'service',
                                params: { id },
                            })
                        "
                    >
                            <td class="font-mono">{{ id }}</td>
                            <td class="font-mono">
                                {{ serviceNameMap?.[id] ?? "?" }}
                            </td>
                            <td class="font-mono">{{ info?.pid }}</td>
                            <td>
                                <CheckSquare
                                    class="text-success"
                                    v-if="info?.active"
                                />
                                <SquareX v-else class="text-failure" />
                            </td>
                            <td>
                                <Badge :class="mapStatusBadge(info?.status)">{{
                                    info?.status
                                }}</Badge>
                            </td>
                            <td>{{ formatDate(info?.start_time) ?? "?" }}</td>
                            <td :style="`min-width: ${uptime_width}ch`">
                                {{
                                    upTime(
                                        info?.start_time,
                                        info?.end_time ?? now,
                                    ) ?? "?"
                                }}
                            </td>
                            <td>{{ info?.restarts }}</td>
                            <td>{{ info?.crashed }}</td>
                    </tr>
                </tbody>
            </table>
        </div>
    </Card>
</template>
