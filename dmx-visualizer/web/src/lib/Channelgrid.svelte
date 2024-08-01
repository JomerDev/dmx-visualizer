<script lang="ts">
    import Channel from "./Channel.svelte";
    export let channels: Array<number>;
</script>

<template>
    <div class="grid">
        {#each channels as value, idx}
            <Channel num={idx + 1} value={value} />
        {/each}
    </div>
</template>

<style>
    .grid {
        display: grid;
        grid-template-columns: repeat(32, 1fr);
        grid-template-rows: repeat(16, 1fr);
        column-gap: 5px;
        row-gap: 5px;
        align-content: stretch;    
        width: 100%;
        padding: 5px;
    }
</style>