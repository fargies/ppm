<script setup>
defineProps({
    title: { type: String },
    skel: { type: Boolean, default: false },
});
</script>
<template>
    <div
        class="inline-block skel-anim py-1 px-3 border border-border rounded-md"
        :class="{ 'skel': skel }"
    >
        <p class="text-xs text-muted-foreground">{{ title }}</p>
        <p class="text-sm flex justify-center align-middle text-foreground">
            <slot />
        </p>
    </div>
</template>
