//! DAC dual mode + TIM6 tetikleme + DMA hedefi.
//!
//! Bu dosya projenin "HAL'in yetmedigi" kismi. stm32h7xx-hal 0.16'nin `dac`
//! modulu bilincli olarak minimal: sadece `enable()` / `set_value()` var;
//! trigger secimi (TSEL), dual mode veya DMA sarmalanmamis durumda. HAL'in
//! DMA altyapisi da DAC icin bir `TargetAddress` implementasyonu icermiyor.
//!
//! Bu yuzden asagida iki yerde PAC (register) seviyesine iniyoruz. Her ikisi
//! de kucuk ve tek seferlik; geri kalan her sey (GPIO, RCC, DMA stream'leri,
//! clock agaci) HAL'in tip-guvenli API'siyle yapiliyor.
//!
//! # Donanim secimlerinin gerekcesi
//!
//! * **Pinler: PA4 = DAC1_OUT1, PA5 = DAC1_OUT2.** Bunlar secim degil, silikon
//!   gercegi -- H750'de DAC cikislari baska pine goturulemez. HAL de bunu
//!   dogruluyor: `Pins<DAC1>` trait'i yalnizca `PA4<Analog>` / `PA5<Analog>`
//!   icin implement edilmis. WeAct board'unda ikisi de header'a cikmis ve
//!   uzerlerinde LED/buton gibi baska bir yuk yok.
//!
//! * **Timer: TIM6.** "Basic timer" -- tek isi periyodik sayip TRGO uretmek.
//!   Capture/compare kanali, dolayisiyla pin cikisi yok; bu yuzden TIM6'yi
//!   kullanmak board'da HICBIR header pinini harcamaz. TIM2/3/4/5 gibi genel
//!   amacli timer'lari PWM/encoder icin serbest birakiyoruz. ST'nin DAC
//!   ornekleri de tam olarak bu sebeple TIM6'yi secer.
//!
//! * **DMA1 Stream 0.** H7'de DMAMUX var, yani herhangi bir istek herhangi bir
//!   stream'e yonlendirilebilir; stream 0 keyfi ama serbest bir secim.

use stm32h7xx_hal::dma::dma::DMAReq;
use stm32h7xx_hal::dma::traits::TargetAddress;
use stm32h7xx_hal::dma::MemoryToPeripheral;
use stm32h7xx_hal::pac;

/// DAC_CR icindeki TSEL alani icin "TIM6 TRGO" degeri.
///
/// # Bu sayi nereden geliyor? (ve PAC'taki bir tuzak)
///
/// ST'nin resmi CMSIS basligi `stm32h743xx.h`:
///
/// ```text
/// #define DAC_CR_TSEL1_Pos   (2U)
/// #define DAC_CR_TSEL1_Msk   (0xFUL << DAC_CR_TSEL1_Pos)   // 0x0000003C
/// ```
///
/// yani alan **4 bit** (5:2) genisliginde ve `stm32h7xx_hal_dac.h`:
///
/// ```text
/// #define DAC_TRIGGER_T6_TRGO  (DAC_CR_TSEL1_2 | DAC_CR_TSEL1_0 | DAC_CR_TEN1)
/// ```
///
/// -> TSEL = 0b0101 = **5** = TIM6 TRGO.
///
/// TUZAK: stm32h7 PAC 0.15.1 bu alani yanlislikla **3 bit** olarak modelliyor
/// (ST'nin kendi basligindaki eski `TSEL1[2:0]` yorumundan gelen bir SVD
/// hatasi). Bizim icin sorun degil, cunku 5 zaten 3 bite sigiyor ve dogru
/// bitlere yaziliyor. Ama TIM15 (8) veya LPTIM1 (11) gibi bir tetige gecmek
/// istersen bu PAC alani sessizce maskeleyip YANLIS deger yazar -- o durumda
/// CR'yi ham `bits()` ile yazman gerekir.
const TSEL_TIM6_TRGO: u8 = 5;

/// DMA'nin yazacagi hedef: DAC1'in `DHR12RD` register'i.
///
/// Bu "birim tipi" (unit struct) calisma aninda hicbir yer kaplamaz (ZST);
/// tek gorevi HAL'in DMA altyapisina "hedef adres nedir, kac bitlik yazilir,
/// hangi DMAMUX istek hattini kullan" bilgisini TIP SEVIYESINDE tasimak.
pub struct DacDualDma;

// SAFETY: `TargetAddress` unsafe bir trait, cunku HAL bizim burada verdigimiz
// adresi ve boyutu dogrulayamaz -- yanlis olursa DMA rastgele bellege yazar.
// Verdigimiz bilgilerin dogrulugu:
//   * `address()` DAC1'in DHR12RD register'inin gercek adresini PAC'tan
//     turetiyor (elle sabit yazmiyoruz).
//   * `MemSize = u32`: DHR12RD 32-bit bir register ve tek yazmada iki kanali
//     birden yukluyor -- senin istedigin "32-bit (word) transfer" bu.
//   * `REQUEST_LINE`: DAC kanal 1'in DMA istek hatti.
unsafe impl TargetAddress<MemoryToPeripheral> for DacDualDma {
    /// Transfer birimi. HAL bundan DMA'nin MSIZE/PSIZE alanlarini
    /// `size_of::<u32>() / 2 = 2` = "word" olarak hesapliyor.
    type MemSize = u32;

    /// DMAMUX1 istek hatti 67 = `dac_ch1_dma`.
    ///
    /// Dual mode'da tek DMA kullandigimiz icin SADECE kanal 1'in istegi
    /// devrede (bkz. `configure_dual_mode`, DMAEN2 kapali). Numarayi elle
    /// yazmiyoruz; PAC'in enum'undan aliyoruz.
    const REQUEST_LINE: Option<u8> = Some(DMAReq::DacCh1Dma as u8);

    fn address(&self) -> usize {
        // SAFETY: Sadece register'in ADRESINI hesapliyoruz, okuma/yazma yok.
        // `addr_of!` referans uretmeden adres alir.
        unsafe { core::ptr::addr_of!((*pac::DAC::ptr()).dhr12rd) as usize }
    }
}

/// DAC1'i dual mode + TIM6 tetikleme + tek DMA icin ayarla.
///
/// HAL'in `.enable()` cagrilarindan ONCE, yani EN1/EN2 hala 0 iken cagrilmali:
/// referans manuel TSEL'in kanal kapaliyken yazilmasini istiyor.
///
/// # Kurulan yapi
///
/// ```text
///   TIM6 ---TRGO---+--> DAC kanal 1 (TEN1=1, TSEL1=5) --> PA4
///                  |         |
///                  |         +--> DMA istegi (DMAEN1=1)
///                  |                    |
///                  |              DMA1 Stream0 (circular)
///                  |                    |
///                  |                    v
///                  |              DHR12RD'ye 32-bit yaz
///                  |                (iki kanali birden yukler)
///                  |
///                  +--> DAC kanal 2 (TEN2=1, TSEL2=5) --> PA5
/// ```
///
/// Ayni TRGO her iki kanali AYNI ANDA tetikler; DMA ise tek bir 32-bit yazma
/// ile ikisinin de tutma register'ini doldurur. Kanallarin ayrisma (drift)
/// ihtimali yok -- iki ayri DMA / iki ayri timer olsaydi olurdu.
///
/// # `DMAEN2` neden kapali?
///
/// Iki kanalda da DMA'yi acsaydik her tetiklemede IKI istek uretilirdi ve DMA
/// LUT'ta iki adim ilerleyip dalgayi bozardi. Dual mode'un butun olayi: kanal
/// 1'in istegi ikisi icin birden veri getiriyor.
pub fn configure_dual_mode() {
    // SAFETY: Bu tek `unsafe` blogun gerekcesi:
    //
    // 1. `DAC::ptr()` -- `dp.DAC` az once HAL'in `.dac()` fonksiyonu tarafindan
    //    tuketildi (o da bize sadece ZST kanal handle'lari geri verdi), yani
    //    register blogunu baska turlu ele gecirmenin yolu yok. HAL'in `dac.rs`
    //    modulu de tam olarak ayni sekilde `&*DAC::ptr()` kullaniyor.
    // 2. Cagiran taraf DAC'in saatini `.dac(...)` ile actigi icin register'lar
    //    erisilebilir durumda (saati kapali bir cevrebirime yazmak fault'tur).
    // 3. `tsel1/tsel2().bits()` unsafe: PAC bu alanlari enum'lamadigi icin
    //    svd2rust degeri dogrulayamiyor. Yazdigimiz 5 degeri gecerli (bkz.
    //    `TSEL_TIM6_TRGO`).
    // 4. Ayni anda baska hicbir kod DAC_CR'ye dokunmuyor: bu fonksiyon
    //    `main`'den, kesmeler baslamadan once, bir kez cagriliyor.
    let dac = unsafe { &*pac::DAC::ptr() };

    dac.cr.modify(|_, w| unsafe {
        w
            // --- Kanal 1: tetik + DMA ---
            .tsel1()
            .bits(TSEL_TIM6_TRGO) // tetik kaynagi = TIM6 TRGO
            .ten1()
            .set_bit() // tetiklemeyi ac
            .dmaen1()
            .set_bit() // her tetikte DMA istegi uret
            // --- Kanal 2: ayni tetik, DMA YOK ---
            .tsel2()
            .bits(TSEL_TIM6_TRGO)
            .ten2()
            .set_bit()
            .dmaen2()
            .clear_bit() // bilincli: tek DMA istegi istiyoruz
    });
}

/// TIM6'yi `sample_rate_hz` frekansinda TRGO uretecek sekilde ayarla.
///
/// Timer'i BASLATMAZ -- DMA hazir olmadan tetik gelmesin diye baslatmayi
/// `start_trigger()`'a birakiyoruz.
///
/// Donus degeri: gerceklesen ornekleme hizi (Hz). Bolme tam olmayabilir,
/// bu yuzden "istedigin" degil "elde edilen" degeri donduruyoruz.
///
/// # Neden PSC/ARR'yi elle hesapliyoruz?
///
/// HAL'in `Timer::set_freq()`'i `arr = ticks / (psc + 1)` yaziyor, ama donanim
/// periyodu `(ARR + 1) * (PSC + 1)` cevrimdir -- yani HAL'de bir off-by-one
/// var ve frekans ~%0.1 kayiyor. Blink LED icin onemsiz, sinyal jeneratoru
/// icin degil.
pub fn configure_tim6(tim6: &pac::TIM6, kernel_clk_hz: u32, sample_rate_hz: u32) -> u32 {
    // Bir ornek icin gereken timer cevrimi (en yakina yuvarlayarak).
    let ticks = ((kernel_clk_hz as u64 + sample_rate_hz as u64 / 2) / sample_rate_hz as u64).max(1);

    // ARR 16-bit (max 65535 -> 65536 cevrim). Daha uzun periyot gerekirse
    // prescaler ile bolelim.
    let psc = ((ticks - 1) / 65_536) as u32;
    let arr = (ticks / (psc as u64 + 1)).clamp(1, 65_536) as u32 - 1;

    // Bu iki cagri `unsafe` DEGIL: PAC bu alanlari `FieldWriterSafe` olarak
    // uretmis, cunku 16-bit alan u16'nin tum degerlerini kabul ediyor.
    tim6.psc.write(|w| w.psc().bits(psc as u16));
    tim6.arr.write(|w| w.arr().bits(arr as u16));

    // MMS = "Update" -> her sayac tasmasinda TRGO darbesi uret.
    // Bu da guvenli: PAC alani `MMS_A::Update` diye enum'lamis.
    tim6.cr2.modify(|_, w| w.mms().update());

    // PSC/ARR "shadow" register'lar: normalde bir sonraki tasmada yururluge
    // girerler. UG=1 ile hemen yukleyip sayaci sifirliyoruz, boylece ilk
    // periyot da dogru uzunlukta oluyor.
    tim6.egr.write(|w| w.ug().set_bit());

    // Gerceklesen hiz.
    kernel_clk_hz / ((psc + 1) * (arr + 1))
}

/// TIM6'yi baslat -- bu andan itibaren DAC ornek uretmeye baslar.
/// Sekil degisiminde LUT tazelendikten sonra "devam et" olarak da kullanilir.
pub fn star