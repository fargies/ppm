<template>
    <div class="border-gray-400 border rounded shadow-sm">
        <div class="w-full flex p-1">
            <slot name="header"></slot>

        </div>
        <slot name="default"></slot>
    </div>
</template>
