#![no_std]
#![no_main]

use dmx_messages::{DMXMessage, DmxTopic, DummyEndpoint};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;

use defmt::unwrap;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    peripherals::{PIO0, UART0, USB},
    pio::InterruptHandler as InterruptHandlerPio,
    uart::{self, Async, DataBits, InterruptHandler as InterruptHandlerUART, Parity, StopBits, Uart},
    usb::{self, Endpoint, Out},
};
// use embassy_time::Timer;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::UsbDevice;
use static_cell::ConstStaticCell;
use {defmt_rtt as _, panic_probe as _};

use postcard_rpc::{define_dispatch, target_server::{
    buffers::AllBuffers, configure_usb, example_config, rpc_dispatch, sender::Sender, Dispatch
}, WireHeader};

bind_interrupts!(struct Irqs {
    UART0_IRQ => InterruptHandlerUART<UART0>;
    USBCTRL_IRQ => InterruptHandler<USB>;
    PIO0_IRQ_0 => InterruptHandlerPio<PIO0>;
});

static ALL_BUFFERS: ConstStaticCell<AllBuffers<1024, 1024, 16>> =
    ConstStaticCell::new(AllBuffers::new());


pub struct Context {}

define_dispatch! {
    dispatcher: Dispatcher<
        Mutex = ThreadModeRawMutex,
        Driver = usb::Driver<'static, USB>,
        Context = Context
    >;
    DummyEndpoint => blocking dummy_enpoint,
}

fn dummy_enpoint(_context: &mut Context, header: WireHeader, rqst: ()) {
    defmt::info!("dummy endpoint: {}", header.seq_no);
    rqst
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let driver = usb::Driver::new(p.USB, Irqs);
    let mut config = example_config();
    config.manufacturer = Some("JomerDev");
    config.product = Some("dmx-reader");
    let buffers = ALL_BUFFERS.take();
    let (device, ep_in, ep_out) = configure_usb(driver, &mut buffers.usb_device, config);
    let dispatch = Dispatcher::new(&mut buffers.tx_buf, ep_in, Context {});
    let sender = dispatch.sender();
    
    
    let mut config = uart::Config::default();
    config.baudrate = 250_000;
    config.data_bits = DataBits::DataBits8;
    config.stop_bits = StopBits::STOP2;
    config.parity = Parity::ParityNone;
    let uart = uart::Uart::new(
        p.UART0, p.PIN_0, p.PIN_1, Irqs, p.DMA_CH0, p.DMA_CH1, config,
    );
    
    defmt::info!("Startup");

    spawner.must_spawn(dispatch_task(ep_out, dispatch, &mut buffers.rx_buf));
    // Run the USB device.
    unwrap!(spawner.spawn(usb_task(device)));
    // Run the uart loop
    unwrap!(spawner.spawn(uart_task(sender, uart)));
    
}

#[embassy_executor::task]
pub async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) {
    usb.run().await;
}

#[embassy_executor::task]
async fn dispatch_task(
    ep_out: Endpoint<'static, USB, Out>,
    dispatch: Dispatcher,
    rx_buf: &'static mut [u8],
) {
    rpc_dispatch(ep_out, dispatch, rx_buf).await;
}

#[embassy_executor::task]
pub async fn uart_task(sender: Sender<ThreadModeRawMutex, Driver<'static, USB>>, mut uart: Uart<'static, UART0, Async>) {
    let mut seq_no: u32 = 0;
    let mut buf1: [u8; 515] = [0; 515];
    let mut buf2: [u8; 515] = [0; 515];
    let mut first = true;
    let mut read;

    let mut res = uart.read_to_break_with_count(&mut buf1, 1).await;
    loop {
        let buf3 = buf1;
        buf1 = buf2;
        buf2 = buf3;
        read = uart.read_to_break_with_count(&mut buf1, 1);
        match res {
            Err(e) => {
                defmt::info!("Error: {}", e);
            }
            Ok(len) => {
                if !first && len > 0 {
                    let mut msg = DMXMessage { channels: [0; 512] };
                    msg.channels[0..len].copy_from_slice(&buf2[1..len + 1]);
                    
                    // If either one of the marked lines is commented out, this line will await forever
                    let e: Result<(), ()> = sender.publish::<DmxTopic>(seq_no, &msg).await;
                    defmt::info!("Sent {} {} {:?}", len, buf2.len(), e);
                    seq_no += 1;
                } else {
                    first = false;
                }
            }
        }
        res = read.await;
    }
}