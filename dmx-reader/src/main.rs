#![no_std]
#![no_main]

use core::array;

use dmx_messages::{DMXMessage, DmxTopic, DummyEndpoint};
use embassy_sync::{blocking_mutex::{raw::{CriticalSectionRawMutex, ThreadModeRawMutex}, CriticalSectionMutex}, channel::Channel, signal::Signal};
use fixed_macro::fixed;
use defmt::unwrap;
use embassy_executor::{Executor, Spawner};
use embassy_rp::{
    bind_interrupts, clocks, dma::{AnyChannel, Channel as DMAChannel}, into_ref, multicore::{spawn_core1, Stack}, peripherals::{PIO0, UART0, USB}, pio::{Common, Config, FifoJoin, Instance, InterruptHandler as InterruptHandlerPio, Pio, PioPin, ShiftConfig, ShiftDirection, StateMachine}, uart::{self, Async, DataBits, InterruptHandler as InterruptHandlerUART, Parity, StopBits, Uart}, usb::{self, Endpoint, Out}, Peripheral, PeripheralRef
};
// use embassy_time::Timer;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::UsbDevice;
use fixed::types::U24F8;
use static_cell::{ConstStaticCell, StaticCell};
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

#[repr(C)]
pub struct RGB<ComponentType> {
    pub r: ComponentType,
    pub g: ComponentType,
    pub b: ComponentType,
}

pub type RGB8 = RGB<u8>;

static mut CORE1_STACK: Stack<8192> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

static SHARED: Signal<CriticalSectionRawMutex, DMXMessage> = Signal::new();

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
    // unwrap!(spawner.spawn(uart_task(sender, uart)));

    spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor1 = EXECUTOR1.init(Executor::new());
            executor1.run(|spawner| unwrap!(spawner.spawn(uart_task(uart))));
        },
    );

    let Pio { mut common, sm0, .. } = Pio::new(p.PIO0, Irqs);

    let mut ws = Ws2812::new(&mut common, sm0, p.DMA_CH2, p.PIN_3);
    let mut seq_no: u32 = 0;
    loop {
        let msg = SHARED.wait().await;
        let _: Result<(), ()> = sender.publish::<DmxTopic>(seq_no, &msg).await;
        seq_no += 1;
        // let rgb: [RGB<u8>; 170] = msg.channels.chunks_exact(3).map(|val| RGB8 { r: val[0], g: val[1], b: val[2] }).collect::<[RGB<u8>; 170]>().try_into().unwrap();
        let rgb: [RGB<u8>; 170] = array::from_fn(|i| {
            let idx = i * 3;
            RGB8 { r: msg.channels[idx], g: msg.channels[idx + 1], b: msg.channels[idx + 2] }
        });
        ws.write(&rgb).await;
    }
    
}

#[embassy_executor::task]
pub async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) {
    usb.run().await;
}

// #[embassy_executor::task]
// pub async fn sender_task(sender: Sender<ThreadModeRawMutex, Driver<'static, USB>>) {
//     usb.run().await;
// }

#[embassy_executor::task]
async fn dispatch_task(
    ep_out: Endpoint<'static, USB, Out>,
    dispatch: Dispatcher,
    rx_buf: &'static mut [u8],
) {
    rpc_dispatch(ep_out, dispatch, rx_buf).await;
}

#[embassy_executor::task]
pub async fn uart_task(mut uart: Uart<'static, UART0, Async>) { // sender: Sender<ThreadModeRawMutex, Driver<'static, USB>>
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

                    let _ = SHARED.signal(msg);
                    
                    // If either one of the marked lines is commented out, this line will await forever
                    

                    // defmt::info!("Sent {} {} {:?}", len, buf2.len(), e);
                    // seq_no += 1;
                } else {
                    first = false;
                }
            }
        }
        res = read.await;
    }
}


pub struct Ws2812<'d, P: Instance, const S: usize, const N: usize> {
    dma: PeripheralRef<'d, AnyChannel>,
    sm: StateMachine<'d, P, S>,
}

impl<'d, P: Instance, const S: usize, const N: usize> Ws2812<'d, P, S, N> {
    pub fn new(
        pio: &mut Common<'d, P>,
        mut sm: StateMachine<'d, P, S>,
        dma: impl Peripheral<P = impl DMAChannel> + 'd,
        pin: impl PioPin,
    ) -> Self {
        into_ref!(dma);

        // Setup sm0

        // prepare the PIO program
        let side_set = pio::SideSet::new(false, 1, false);
        let mut a: pio::Assembler<32> = pio::Assembler::new_with_side_set(side_set);

        const T1: u8 = 2; // start bit
        const T2: u8 = 5; // data bit
        const T3: u8 = 3; // stop bit
        const CYCLES_PER_BIT: u32 = (T1 + T2 + T3) as u32;

        let mut wrap_target = a.label();
        let mut wrap_source = a.label();
        let mut do_zero = a.label();
        a.set_with_side_set(pio::SetDestination::PINDIRS, 1, 0);
        a.bind(&mut wrap_target);
        // Do stop bit
        a.out_with_delay_and_side_set(pio::OutDestination::X, 1, T3 - 1, 0);
        // Do start bit
        a.jmp_with_delay_and_side_set(pio::JmpCondition::XIsZero, &mut do_zero, T1 - 1, 1);
        // Do data bit = 1
        a.jmp_with_delay_and_side_set(pio::JmpCondition::Always, &mut wrap_target, T2 - 1, 1);
        a.bind(&mut do_zero);
        // Do data bit = 0
        a.nop_with_delay_and_side_set(T2 - 1, 0);
        a.bind(&mut wrap_source);

        let prg = a.assemble_with_wrap(wrap_source, wrap_target);
        let mut cfg = Config::default();

        // Pin config
        let out_pin = pio.make_pio_pin(pin);
        cfg.set_out_pins(&[&out_pin]);
        cfg.set_set_pins(&[&out_pin]);

        cfg.use_program(&pio.load_program(&prg), &[&out_pin]);

        // Clock config, measured in kHz to avoid overflows
        // TODO CLOCK_FREQ should come from embassy_rp
        let clock_freq = U24F8::from_num(clocks::clk_sys_freq() / 1000);
        let ws2812_freq = fixed!(800: U24F8);
        let bit_freq = ws2812_freq * CYCLES_PER_BIT;
        cfg.clock_divider = clock_freq / bit_freq;

        // FIFO config
        cfg.fifo_join = FifoJoin::TxOnly;
        cfg.shift_out = ShiftConfig {
            auto_fill: true,
            threshold: 24,
            direction: ShiftDirection::Left,
        };

        sm.set_config(&cfg);
        sm.set_enable(true);

        Self {
            dma: dma.map_into(),
            sm,
        }
    }

    pub async fn write(&mut self, colors: &[RGB8; N]) {
        // Precompute the word bytes from the colors
        let mut words = [0u32; N];
        for i in 0..N {
            let word = (u32::from(colors[i].g) << 24)
                | (u32::from(colors[i].r) << 16)
                | (u32::from(colors[i].b) << 8);
            words[i] = word;
        }

        // DMA transfer
        self.sm.tx().dma_push(self.dma.reborrow(), &words).await;
    }
}