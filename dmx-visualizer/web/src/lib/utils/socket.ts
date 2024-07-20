
const _rid = Symbol("[[rid]]");
const _waiting = Symbol("[[waiting]]");
const _sendQueue = Symbol("[[sendQueue]]");
const _services = Symbol("[[services]]");


export class Socket extends WebSocket {
    [_rid]: number = 1;
    [_waiting]: Record<number, { resolve: (value: unknown) => void, reject: (reason?: any) => void }> = {};
    [_sendQueue]: string[] = [];

    constructor() {
        if ((window as any).wsocket) {
            return (window as any).wsocket;
        }

        var str = window.location.host
        str = str.substring( 0, str.indexOf( ":" ) );
        if( !str || str == "" ) {
            str = "localhost"
        }
        super("wss://" + str + ":8080/ws");

        (window as any).wsocket = this;

        this.onmessage = (e: MessageEvent) => {
            const data: ServiceResponse = JSON.parse(e.data);
            if (data.request_id !== undefined){
                if (this[_waiting][data.request_id]) {
                    this[_waiting][data.request_id].resolve(data.response);
                } else if(data.request_id === 0) {
                    this.dispatchEvent(new CustomEvent(`DMXMessage`, {detail: data.response}));
                } else {
                    this[_services][data.service](data.response);
                }
            }
        }

        this.addEventListener("open", () => {
            this[_sendQueue].forEach(s => {
                this.send(s);
            });
        });
    }

    sendWhenReady( s: string) {
        if (this.readyState !== this.OPEN) {
            this[_sendQueue].push(s);
        } else {
            return this.send(s);
        }
    }

    async isOpen() {
        if (this.readyState !== this.OPEN) {
            return new Promise( ( resolve, reject ) => this.addEventListener("open", resolve, { once: true }));
        }
    }
}

export const socket = new Socket();