<template>
    <div class="border-border border rounded shadow-sm">
        <div class="w-full flex p-1">
            <slot name="header"></slot>

        </div>
        <slot name="default"></slot>
    </div>
</template>
