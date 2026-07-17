//! WeAct STM32H750VBT6 -- osiloskop XY vektor cizici.
//!
//! Birkac hazir sekli (elips, cember, sekiz, kalp, yildiz, gul, kelebek,
//! spiral, disli) DAC dual mode + TIM6 tetigi + DMA ile ureti. Osiloskobu XY
//! moduna al: PA4 = yatay,
//! PA5 = dikey.
//!
//! * SD karttaki `SINUS.CFG` -> `shape = ...` baslangic seklini secer.
//! * K1 butonu (PC13) sekiller arasinda gecer.
//! * User LED (PE3) o an kacinci sekilde oldugunu yanip sonerek gosterir.
//!
//! Flashleme: `README.md` (USB DFU / dfu-util veya STM32CubeProgrammer).

#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;

use stm32h7xx_hal::dma::{
    dma::{DmaConfig, StreamsTuple},
    MemoryToPeripheral, Transfer,
};
use stm32h7xx_hal::hal::blocking::delay::DelayMs;
use stm32h7xx_hal::hal::digital::v2::OutputPin;
use stm32h7xx_hal::rcc::{self, ResetEnable};
use stm32h7xx_hal::{pac, prelude::*};

mod config;
mod dac_dma;
mod sdcard;
mod shapes;
mod waveform;

use config::Config;
use dac_dma::DacDualDma;

/// WeAct MiniSTM32H7xx board'unda takili kristal.
const HSE_FREQ_MHZ: u32 = 25;

/// SD okunamazsa kullanilan gomulu ayarlar. `SINUS.CFG` ile ayni format.
///
/// Baslangic sekli KALP -- SD kart takili olmasa da board acilir acilmaz
/// osiloskop XY ekranina kalp cizer. Diger sekillere K1 butonu (PC13) ile
/// gecilir.
const FALLBACK_CONFIG: &str = "\
# WeAct H750 -- osiloskop XY sekil cizici
# shape: ellipse|circle|figure8|heart|star|rose|butterfly|spiral|gear
shape     = heart
amplitude = 0.85
offset    = 0.5
frame_hz  = 60
";

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    // SysTick'ten gecikme uretecegiz (SD init beklemesi, buton debounce, LED).
    let cp = cortex_m::Peripherals::take().unwrap();

    // --- Guc + saat (H750 icin VOS1 @ 400 MHz; sdmmc icin pll1_q) ----------
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .use_hse(HSE_FREQ_MHZ.MHz())
        .sys_ck(400.MHz())
        .hclk(200.MHz())
        .pclk1(100.MHz())
        .pll1_strategy(rcc::PllConfigStrategy::Iterative)
        .pll1_q_ck(100.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    // --- GPIO port bolme ---------------------------------------------------
    // GPIOC hem SDMMC veri hatlarini (PC8..PC12) hem de K1 butonunu (PC13)
    // barindiriyor; bir kere bolup ayri ayri pinleri aliyoruz.
    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    let mut delay = cp.SYST.delay(ccdr.clocks);

    // --- Ayarlar: SD karttan, olmazsa gomulu ------------------------------
    let mut cfg_buf = [0u8; sdcard::MAX_CONFIG_BYTES];
    let cfg = match sdcard::try_read_config(
        dp.SDMMC1,
        (
            gpioc.pc12.into_alternate(), // CLK
            gpiod.pd2.into_alternate(),  // CMD
            gpioc.pc8.into_alternate(),  // D0
            gpioc.pc9.into_alternate(),  // D1
            gpioc.pc10.into_alternate(), // D2
            gpioc.pc11.into_alternate(), // D3
        ),
        ccdr.peripheral.SDMMC1,
        &ccdr.clocks,
        &mut delay,
        &mut cfg_buf,
    ) {
        Some(n) => Config::parse_bytes(&cfg_buf[..n]),
        None => Config::parse_bytes(FALLBACK_CONFIG.as_bytes()),
    };

    // O an cizilen sekil. Butonla degisecegi icin `mut`.
    let mut shape = cfg.shape;

    // --- LUT'u ilk sekille doldur (DMA kurulmadan ONCE) -------------------
    waveform::fill(shape, &cfg);
    let lut = waveform::take().unwrap();

    // --- DAC: PA4 / PA5 -> analog, dual mode + tetik + DMA ----------------
    let dac_pins = (gpioa.pa4.into_analog(), gpioa.pa5.into_analog());
    let (dac1, dac2) = dp.DAC.dac(dac_pins, ccdr.peripheral.DAC12);
    dac_dma::configure_dual_mode();
    let _dac1 = dac1.enable();
    let _dac2 = dac2.enable();

    // --- TIM6: frame_hz * POINTS hizinda tetik ----------------------------
    let _ = ccdr.peripheral.TIM6.enable().reset();
    let sample_rate = (cfg.frame_hz * waveform::POINTS as f32) as u32;
    let _actual = dac_dma::configure_tim6(&dp.TIM6, ccdr.clocks.timx_ker_ck().raw(), sample_rate);

    // --- DMA1 Stream 0: bellek -> DAC, dairesel ---------------------------
    let streams = StreamsTuple::new(dp.DMA1, ccdr.peripheral.DMA1);
    let dma_config = DmaConfig::default()
        .memory_increment(true)
        .peripheral_increment(false)
        .circular_buffer(true)
        .fifo_enable(false)
        .priority(stm32h7xx_hal::dma::config::Priority::High);

    let mut transfer: Transfer<_, _, MemoryToPeripheral, _, _> =
        Transfer::init(streams.0, DacDualDma, lut, None, dma_config);
    transfer.start(|_| {});

    // --- Baslat -----------------------------------------------------------
    dac_dma::start_trigger(&dp.TIM6);

    // --- Buton + LED dongusu ----------------------------------------------
    // PC13 = K1: aktif-high, pull-down (WeAct/Zephyr board tanimi). Basilinca
    // high okunur, o yuzden pull-down input.
    let button = gpioc.pc13.into_pull_down_input();
    // PE3 = user LED.
    let mut led = gpioe.pe3.into_push_pull_output();

    // Acilista o anki sekli LED ile duyur (kacinci sekil = kac yanip sonme).
    flash(&mut led, &mut delay, shape.index() + 1);

    let mut last_pressed = false;
    let mut hb: u32 = 0; // heartbeat sayaci
    let mut led_on = false;

    loop {
        let pressed = button.is_high();

        // Yukselen kenar = yeni bir basis. `pressed && !last_pressed` ile
        // butonu basili TUTMAK tek gecis sayilir, surekli degil.
        if pressed && !last_pressed {
            shape = shape.next();

            // Sekli guvenle degistir: timer'i durdur, LUT'u tazele, devam et.
            // (DMA ayni tampon/uzunlukla calismaya devam eder.)
            dac_dma::pause_trigger(&dp.TIM6);
            waveform::fill(shape, &cfg);
            dac_dma::start_trigger(&dp.TIM6);

            // Yeni seklin numarasini LED ile bildir.
            flash(&mut led, &mut delay, shape.index() + 1);
            led_on = false;

            // Kaba debounce: basistan sonra kisa bir bekleme, geri sekmelerin
            // ikinci bir gecis olarak sayilmasini onler.
            delay.delay_ms(150u32);
        }
        last_pressed = pressed;

        // "Yasiyorum" kalp atisi: ~16 * 25ms = ~400ms'de bir LED durum degistir.
        hb = hb.wrapping_add(1);
        if hb % 16 == 0 {
            led_on = !led_on;
            let _ = if led_on { led.set_high() } else { led.set_low() };
        }

        delay.delay_ms(25u32);
    }
}

/// LED'i `times` kez hizlica yakip sondur (sekil numarasini bildirmek icin).
///
/// `impl OutputPin` sayesinde somut pin tipini yazmak zorunda kalmiyoruz;
/// `set_high/set_low` embedded-hal 0.2'de `Result` doner, `let _` ile
/// yutuyoruz (hata donmez, tip Infallible).
fn flash(led: &mut impl OutputPin, delay: &mut impl DelayMs<u32>, times: usize) {
    let _ = led.set_low();
    delay.delay_ms(200u32);
    for _ in 0..times {
        let _ = led.set_high();
        delay.delay_ms(130u32);
        let _ = led.set_low();
        delay.delay_ms(170u32);
    }
    delay.delay_ms(250u32);
}
