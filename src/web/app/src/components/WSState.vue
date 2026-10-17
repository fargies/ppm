<script setup>
import { watch, getCurrentInstance, nextTick, ref } from "vue";
import { ArrowBigDownDash, ChevronsLeftRightEllipsis } from "@lucide/vue";

const { status } = defineProps(['status'])

let show = ref(false);

watch(() => status, function () {
  show.value = false;
  nextTick(() => show.value = true)
})

</script>
<style scoped>
@keyframes FxFadeAnimation {
    0% { opacity: 1; color: var(--color-green-800); }
    10% { opacity: 0.5; color: var(--color-green-800); }
    20% { opacity: 1; color: var(--color-green-800); }
    100% { opacity: 0; }
}

.fx-fade {
    animation-name: FxFadeAnimation;
    animation-duration: 2s;
    animation-fill-mode: forwards;
}
</style>
<template>
    <span class="inline-block fx-fade" v-if="show">
        <ArrowBigDownDash />
    </span>
</template>
