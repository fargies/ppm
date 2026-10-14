<style>
@reference "../style.css";

.badge.success {
    background-color: var(--success);
    color: var(--foreground);
    border-color: transparent;
}

.badge.failure {
    background-color: var(--color-destructive);
    color: var(--color-destructive-foreground);
    border-color: transparent;
}

.badge {
    @apply bg-foreground/5;
    @apply text-foreground;
    @apply font-semibold;
    @apply border-border;
}

</style>
<template>
    <span
        class="badge inline-flex w-fit shrink-0 items-center justify-center gap-1 rounded-md border px-2 py-0.5 text-xs whitespace-nowrap"
    >
        <slot></slot>
    </span>
</template>
