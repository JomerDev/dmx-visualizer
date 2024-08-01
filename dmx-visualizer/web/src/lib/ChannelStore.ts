import { writable } from "svelte/store";
import type { DMXMessage } from "../exports/DMXMessage";
import { socket } from "./utils/socket";


function createChannelStore() {
    const {set, update, subscribe } = writable<Array<number>>(Array(512), (_set, update) => {

        const onDMXMessage = (evt: any) => {
            let channels = (evt as CustomEvent<Array<number>>).detail;
            if (channels) {
                channels.length = 512;
                update(value => {
                    return [
                        ...channels
                    ]
                })
            }
        };
        socket.addEventListener("DMXMessage", onDMXMessage);

        return () => {
            socket.removeEventListener("DMXMessage", onDMXMessage);
        }
    });

    return {
        subscribe,
    };
}

const channelStore = createChannelStore();


export default channelStore;