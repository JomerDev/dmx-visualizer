<script lang="ts">

    export let num: number;
    export let value: number;
</script>

<template>
    <div class="outer" >
        {num}/{value || 0}
        <div class="inner" style:background-color={`rgba(255,0,0,${1/255*value})`}>
        </div>
    </div>
</template>

<style>
    .outer {
        display: flex;
        flex: 1 1 auto;
        width: 100%;
        height: 100%;
        /* background-color: red; */
        font-size: small;
        justify-content: center;
        flex-direction: column;
    }

    .inner {
        width: 100%;
        height: 100%;
    }
</style>