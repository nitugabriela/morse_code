#![no_std]
#![no_main]
#![allow(unused_imports)]

use core::panic::PanicInfo;
use core::str::from_utf8;

use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_time::{Duration, Timer, Delay};
use log::{info, warn};
use static_cell::StaticCell;

use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler as USBInterruptHandler};

use heapless::Vec;
use heapless::String;

use embassy_rp::peripherals::*;
use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use embassy_rp::usb::{Driver as UsbDriver, InterruptHandler as UsbInterruptHandler};

use ag_lcd::{LcdDisplay, Cursor};

use embassy_rp::i2c::I2c;
use port_expander::Pcf8574;
use embassy_rp::i2c::Config as I2cConfig;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => USBInterruptHandler<USB>;
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

const WIFI_NETWORK: &str = "Ela-iPhone";
const WIFI_PASSWORD: &str = "ela.catut";

#[embassy_executor::task]
async fn logger_task(driver: UsbDriver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

const DISPLAY_FREQ: u32 = 200_000;
const TOP: u16 = 0x8000;
const MORSE_CODE_TABLE: [&str; 36] = [
    ".-",   // A
    "-...", // B
    "-.-.", // C
    "-..",  // D
    ".",    // E
    "..-.", // F
    "--.",  // G
    "....", // H
    "..",   // I
    ".---", // J
    "-.-",  // K
    ".-..", // L
    "--",   // M
    "-.",   // N
    "---",  // O
    ".--.", // P
    "--.-", // Q
    ".-.",  // R
    "...",  // S
    "-",    // T
    "..-",  // U
    "...-", // V
    ".--",  // W
    "-..-", // X
    "-.--", // Y
    "--..", // Z
    "-----", // 0
    ".----", // 1
    "..---", // 2
    "...--", // 3
    "....-", // 4
    ".....", // 5
    "-....", // 6
    "--...", // 7
    "---..", // 8
    "----." // 9
];

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());

    let driver = UsbDriver::new(peripherals.USB, Irqs);
    spawner.spawn(logger_task(driver)).unwrap();

    let mut config_pwm: PwmConfig = Default::default();
    config_pwm.top = 0xFFFF;
    config_pwm.compare_b = 0;

    let mut buzzer = Pwm::new_output_b(peripherals.PWM_SLICE3, peripherals.PIN_7, config_pwm.clone());

    let mut config_red: PwmConfig = Default::default();
    config_red.top = TOP;
    config_red.compare_b = config_red.top;

    let mut config_green: PwmConfig = Default::default();
    config_green.top = TOP;
    config_green.compare_a = 0;

    let mut config_blue: PwmConfig = Default::default();
    config_blue.top = TOP;
    config_blue.compare_a = 0;

    let mut pwm_red = Pwm::new_output_b(peripherals.PWM_SLICE0, peripherals.PIN_1, config_red.clone());
    let mut pwm_green =
        Pwm::new_output_a(peripherals.PWM_SLICE1, peripherals.PIN_2, config_green.clone());
    let mut pwm_blue =
        Pwm::new_output_a(peripherals.PWM_SLICE2, peripherals.PIN_4, config_blue.clone());

    let sda = peripherals.PIN_20;
    let scl = peripherals.PIN_21;

    let delay = Delay;

    let mut config = I2cConfig::default();
    config.frequency = DISPLAY_FREQ;

    let i2c = I2c::new_blocking(peripherals.I2C0, scl, sda, config.clone());
    let mut i2c_expander = Pcf8574::new(i2c, true, true, true);

    let mut lcd: LcdDisplay<_, _> = LcdDisplay::new_pcf8574(&mut i2c_expander, delay)
        .with_cursor(Cursor::Off)
        .with_reliable_init(10000)
        .build();

    let pwr = Output::new(peripherals.PIN_23, Level::Low);
    let cs = Output::new(peripherals.PIN_25, Level::High);
    let mut pio = Pio::new(peripherals.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        peripherals.PIN_24,
        peripherals.PIN_29,
        peripherals.DMA_CH0,
    );

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    spawner.spawn(wifi_task(runner)).unwrap();

    control.init(clm).await;
    control.set_power_management(cyw43::PowerManagementMode::PowerSave).await;

    let config = Config::dhcpv4(Default::default());
    let seed = 0x0123_4567_89ab_cdef;

    static STACK: StaticCell<Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        net_device,
        config,
        RESOURCES.init(StackResources::<2>::new()),
        seed,
    ));

    spawner.spawn(net_task(stack)).unwrap();

    loop {
        match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status {}", err.status);
            }
        }
    }

    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up {:?}!", stack.config_v4());

    let mut rx_buffer = [0; 4096];
    let mut rx_metadata_buffer = [PacketMetadata::EMPTY; 3];
    let mut tx_buffer = [0; 4096];
    let mut tx_metadata_buffer = [PacketMetadata::EMPTY; 3];

    let mut buf = [0u8; 4096];

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_metadata_buffer,
        &mut rx_buffer,
        &mut tx_metadata_buffer,
        &mut tx_buffer,
    );

    info!("Starting server on UDP:1234...");

    if let Err(e) = socket.bind(1234) {
        info!("bind error: {:?}", e);
    }

    loop {
        let r = socket.recv_from(&mut buf).await;
        match r {
            Ok((n, _peer)) => {
                let input_word = match from_utf8(&buf[..n]) {
                    Ok(v) => v,
                    Err(e) => {
                        info!("Invalid UTF-8 sequence: {}", e);
                        continue;
                    }
                };

                info!("Received word: {}", input_word);

                let mut morse_code = Vec::<(char, &str), 32>::new();
                for c in input_word.chars() {
                    match c {
                        'A'..='Z' => {
                            let code = MORSE_CODE_TABLE[(c as u8 - b'A') as usize];
                            morse_code.push((c, code)).unwrap();
                        }
                        'a'..='z' => {
                            let code = MORSE_CODE_TABLE[(c as u8 - b'a') as usize];
                            morse_code.push((c, code)).unwrap();
                        }
                        '0'..='9' => {
                            let code = MORSE_CODE_TABLE[(c as u8 - b'0' + 26) as usize];
                            morse_code.push((c, code)).unwrap();
                        }
                        ' ' => {
                            morse_code.push((' ', "")).unwrap();
                        }
                        _ => (),
                    }
                }

                lcd.clear();
                lcd.print("START CONVERSION");

                Timer::after(Duration::from_millis(1000)).await;

                for (i, (letter, code)) in morse_code.iter().enumerate() {
                    lcd.clear();
                    let mut display_string = String::<128>::new();
                    display_string.push(*letter).unwrap();
                    if i != morse_code.len() - 1 && *letter != ' ' {
                        display_string.push(' ').unwrap();
                        display_string.push('=').unwrap();
                        display_string.push(' ').unwrap();
                    }

                    lcd.print(&display_string);

                    Timer::after(Duration::from_millis(500)).await;

                    for symbol in code.chars() {
                        lcd.clear();
                        display_string.push(symbol).unwrap();
                        display_string.push(' ').unwrap();

                        lcd.print(&display_string);

                        match symbol {
                            '.' => {
                                config_red.compare_b = TOP;
                                config_green.compare_a = 0;
                                config_blue.compare_a = 0;
                                config_pwm.compare_b = config_pwm.top / 4;
                            }
                            '-' => {
                                config_red.compare_b = TOP;
                                config_green.compare_a = TOP;
                                config_blue.compare_a = 0;
                                config_pwm.compare_b = config_pwm.top / 2;
                            }
                            _ => {
                                config_red.compare_b = 0;
                                config_green.compare_a = 0;
                                config_blue.compare_a = TOP;
                            }
                        }

                        pwm_red.set_config(&config_red);
                        pwm_green.set_config(&config_green);
                        pwm_blue.set_config(&config_blue);
                        buzzer.set_config(&config_pwm);

                        Timer::after(Duration::from_millis(if symbol == '.' { 100 } else { 300 })).await;

                        pwm_red.set_config(&Default::default());
                        pwm_green.set_config(&Default::default());
                        pwm_blue.set_config(&Default::default());
                        buzzer.set_config(&Default::default());
                    }

                    if i != morse_code.len() - 1 {
                        lcd.clear();
                        Timer::after(Duration::from_millis(500)).await;
                    }
                }

                lcd.clear();
                lcd.print("TRANSLATION DONE");

                config_red.compare_b = 0;
                config_green.compare_a = 0;
                config_blue.compare_a = TOP;

                pwm_red.set_config(&config_red);
                pwm_green.set_config(&config_green);
                pwm_blue.set_config(&config_blue);

                Timer::after(Duration::from_millis(1000)).await;
            }
            Err(_) => {
                info!("An error occurred when receiving the packet!");
            }
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("Panic occurred: {:?}", info);
    loop {}
}

