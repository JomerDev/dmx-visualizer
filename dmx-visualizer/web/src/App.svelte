<script>
    import Channelgrid from "./lib/Channelgrid.svelte";
    import channelStore from "./lib/ChannelStore";
</script>

<main>
    <Channelgrid channels={$channelStore} />
</main>

<style>
    main {
        display: flex;
        width: 100%;
        height: 100%;
    }
</style>
