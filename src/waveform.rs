//! DMA'nin okudugu LUT tamponu ve onu sekille doldurma.
//!
//! Onemli tasarim karari: **LUT uzunlugu sabit** (`POINTS`). Sekil
//! degistiginde tamponun ADRESI ve UZUNLUGU degismiyor, sadece ICERIGI
//! yeniden yaziliyor. Boylece butona her basista DMA'yi yeniden yapilandirmak
//! gerekmiyor -- TIM6'yi bir an durdurup icerigi tazeleyip devam ettiriyoruz.
//! DMA ayni tampondan okumaya kesintisiz devam eder.

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::config::Config;
use crate::shapes::Shape;

/// LUT'taki nokta sayisi. Tum sekiller bu kadar noktaya orneklenir.
///
/// 1024, kalp/yildiz gibi koseli sekilleri bile puruzsuz gosterecek kadar
/// bol. Timer tetik hizi = frame_hz * POINTS (orn. 60 * 1024 ~ 61 kHz),
/// DAC icin cok rahat. 1024 * 4 = 4 KB, AXI SRAM'de devede kulak.
pub const POINTS: usize = 1024;

const DAC_MAX: f32 = 4095.0;

/// DMA tamponu -- AXI SRAM'de (DMA DTCM'ye erisemez, bkz. `memory.x`).
#[link_section = ".axisram"]
static mut LUT: MaybeUninit<[u32; POINTS]> = MaybeUninit::uninit();

/// `take()` daha once cagrildi mi? `&'static mut` yalnizca bir kez uretilmeli.
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Bir kanal ornegini (-1..=1) 12-bit DAC koduna cevir (genlik + offset ile).
fn code(sample: f32, cfg: &Config) -> u32 {
    let center = cfg.offset * DAC_MAX;
    let peak = cfg.amplitude * (DAC_MAX / 2.0);
    let v = center + peak * sample;
    if v <= 0.0 {
        0
    } else if v >= DAC_MAX {
        4095
    } else {
        (v + 0.5) as u32 // en yakina yuvarla
    }
}

/// Iki kanal kodunu tek bir `DHR12RD` kelimesinde birlestir.
/// bit 0..11 = kanal 1 (PA4 = x), bit 16..27 = kanal 2 (PA5 = y).
#[inline]
fn word(x: u32, y: u32) -> u32 {
    (y << 16) | x
}

/// LUT'u secili sekille doldur.
///
/// Hem acilista (ilk doldurma) hem de her buton basisinda (sekil degisimi)
/// cagrilir. Ikinci ve sonraki cagrilar, DMA'nin okudugu tamponu YERINDE
/// yeniden yazar.
///
/// # SAFETY
///
/// `addr_of_mut!` ile ham isaretci uzerinden yaziyoruz. Bu blogun guvenli
/// olmasinin nedeni:
///
/// * Yazma araligi `0..POINTS`, yani her zaman tamponun icinde.
/// * Sekil degisiminde cagiran taraf (`main`) once TIM6'yi DURDURUYOR, yani
///   DMA o an tampondan okumuyor -- yazma ile okuma cakismiyor.
/// * D-cache kapali (hic acmadik), dolayisiyla CPU'nun yazdigi degerler
///   DMA'ya aninda gorunur; cache temizligi gerekmiyor.
/// * Tek cekirdek, kesme yok: bu fonksiyon calisirken baska kimse LUT'a
///   dokunmuyor.
pub fn fill(shape: Shape, cfg: &Config) {
    let ptr = core::ptr::addr_of_mut!(LUT) as *mut u32;
    for i in 0..POINTS {
        let (x, y) = shape.sample(i, POINTS);
        unsafe { ptr.add(i).write(word(code(x, cfg), code(y, cfg))) };
    }
}

/// DMA'ya verilecek `&'static mut` dilim -- yalnizca ilk cagri `Some` doner.
///
/// Dilim (`&mut [u32]`), sabit dizi degil: DMA transfer sayisi dilimin
/// uzunlugundan (`POINTS`) belirlenir. Ilerde farkli uzunluk istersek burada
/// bir alt-dilim dondurmek yeterli olur.
///
/// Cagirmadan ONCE en az bir kez `fill()` yapilmis olmali; yoksa DMA
/// ilklendirilmemis bellek okur.
pub fn take() -> Option<&'static mut [u32]> {
    if TAKEN.swap(true, Ordering::SeqCst) {
        return None;
    }
    // SAFETY: Tek sahiplik `TAKEN` ile garantili; `fill()` cagrildigi icin
    // bellek gecerli bir `[u32; POINTS]`.
    let arr = unsafe { &mut *(core::ptr::addr_of_mut!(LUT) as *mut [u32; POINTS]) };
    Some(&mut arr[..])
}
