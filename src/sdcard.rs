//! SD karttan config dosyasi okuma (SDMMC1 + FAT32).
//!
//! WeAct MiniSTM32H7xx board'undaki microSD yuvasi **SDMMC1**'e bagli:
//!
//! | Sinyal | Pin  | Alternate function |
//! |--------|------|--------------------|
//! | CLK    | PC12 | AF12 |
//! | CMD    | PD2  | AF12 |
//! | D0     | PC8  | AF12 |
//! | D1     | PC9  | AF12 |
//! | D2     | PC10 | AF12 |
//! | D3     | PC11 | AF12 |
//!
//! (Pin listesi tahmin degil: HAL'in `sdmmc.rs` icindeki `pins!` makrosu
//! SDMMC1 icin tam olarak bunlari tanimliyor.)
//!
//! # Buradaki tampon neden DTCM'de olabiliyor?
//!
//! LUT icin "DMA DTCM'ye erisemez" diye ozel bir bellek bolumu acmistik
//! (`memory.x`). Burada oyle bir dert YOK, cunku stm32h7xx-hal'in SDMMC
//! okumasi DMA kullanmiyor: CPU, SDMMC'nin FIFO register'ini poll edip
//! kelime kelime kopyaliyor (`sdmmc.rs` -> `read_block`). Veriyi CPU tasidigi
//! icin tampon CPU'nun erisebildigi her yerde -- stack dahil -- durabilir.
//!
//! Kural boyle: **veriyi kim tasiyor?** CPU ise DTCM serbest, DMA ise degil.

use embedded_sdmmc::{Controller, Mode, TimeSource, Timestamp, VolumeIdx};
use stm32h7xx_hal::gpio::{gpioc, gpiod, Alternate};
// `stm32h7xx_hal::hal` = HAL'in yeniden disa aktardigi embedded-hal 0.2.
// Kendi Cargo.toml'umuza embedded-hal eklemiyoruz: yanlis surumu secip
// HAL'inkiyle catismasi isten bile degil. Hep HAL'in verdigini kullan.
use stm32h7xx_hal::hal::blocking::delay::DelayMs;
use stm32h7xx_hal::rcc::{rec, CoreClocks};
use stm32h7xx_hal::sdmmc::{SdCard, Sdmmc, SdmmcExt};
use stm32h7xx_hal::{pac, time::Hertz};

/// SD kartin kokunde aranan dosya.
///
/// embedded-sdmmc 0.5 klasik 8.3 kisa isim kullanir; `SINUS.CFG` (5+3) uyuyor.
/// Uzun isim verirsen (`sinus_config.txt`) dosya BULUNAMAZ.
pub const CONFIG_FILENAME: &str = "SINUS.CFG";

/// SDMMC1'in 4-bit modda kullandigi pinler: `(CLK, CMD, D0, D1, D2, D3)`.
///
/// Sira onemli -- HAL `Pins` trait'ini bu tuple duzeni icin implement etmis.
/// Yanlis sirada verirsen derleme hatasi alirsin (calisma aninda sessiz bir
/// bug degil), cunku her pin kendi trait'ini (`PinClk`, `PinCmd`, ...) tasiyor.
pub type Sdmmc1Pins = (
    gpioc::PC12<Alternate<12>>,
    gpiod::PD2<Alternate<12>>,
    gpioc::PC8<Alternate<12>>,
    gpioc::PC9<Alternate<12>>,
    gpioc::PC10<Alternate<12>>,
    gpioc::PC11<Alternate<12>>,
);

/// SD kart veri yolu hizi.
///
/// Dusuk tutuyoruz: acilista bir kez ~100 bayt okuyoruz, hiz tamamen
/// onemsiz; guvenilirlik onemli. Kart uyumsuzlugu yasarsan burayi daha da
/// dusurebilirsin (HAL ornegi de ayni sebeple 2 MHz kullaniyor).
const SD_BUS_FREQ: u32 = 2_000_000;

/// Kart oturana kadar kac deneme yapilacagi.
const INIT_RETRIES: u32 = 3;

/// Config dosyasi icin ust sinir (bayt).
///
/// `try_read_config`'e verilen tampon en az bu kadar olmali. Dosya bundan
/// buyukse dosya YOK SAYILIR (bkz. asagidaki uzunluk kontrolu).
///
/// 1 KB, birkac satir `anahtar = deger` ve bol bol yorum icin fazlasiyla
/// yeterli; stack'te durdugu icin de bedeli yok.
pub const MAX_CONFIG_BYTES: usize = 1024;

/// FAT dosya damgalari icin sahte saat.
///
/// FAT her dosyaya olusturma/degistirme tarihi yazar, bu yuzden
/// embedded-sdmmc bir `TimeSource` istiyor. Biz dosyayi sadece OKUYORUZ,
/// yani bu deger hicbir yere yazilmiyor. Gercek tarih isteseydin RTC'yi
/// baglaman gerekirdi.
struct NoTimeSource;

impl TimeSource for NoTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

/// SD karttaki `SINUS.CFG` dosyasini `buf`'a oku.
///
/// Basarili olursa okunan bayt sayisini dondurur. **Hicbir durumda panik
/// etmez ve sonsuza kadar beklemez.** `None` doner eger: kart yok, FAT32
/// degil, dosya yok, dosya `buf`'tan buyuk, veya okuma hata verdi.
///
/// `buf` en az [`MAX_CONFIG_BYTES`] uzunlugunda olmali.
///
/// # Neden `Option`, neden `Result` degil?
///
/// Cagiran taraf icin hatalar arasindaki fark anlamsiz: kart yok da olsa
/// dosya yok da olsa yapilacak sey ayni -- gomulu varsayilanlara don ve
/// sinyali uretmeye devam et. Ekranimiz/log'umuz olmadigi icin hangi hata
/// oldugunu zaten gosteremezdik. Bir sinyal ureteci, config dosyasi diye
/// acilmamazlik etmemeli.
///
/// Hata ayiklamak istersen `.ok()?` yerine `match` koyup her adimi bir LED
/// yanip sonmesiyle isaretleyebilirsin.
pub fn try_read_config(
    sdmmc: pac::SDMMC1,
    pins: Sdmmc1Pins,
    prec: rec::Sdmmc1,
    clocks: &CoreClocks,
    delay: &mut impl DelayMs<u32>,
    buf: &mut [u8],
) -> Option<usize> {
    // Tip anotasyonu sart: HAL ayni surucuyle hem SD kart hem eMMC
    // destekliyor, `SdCard` diyerek hangisini istedigimizi soyluyoruz.
    let mut sd: Sdmmc<_, SdCard> = sdmmc.sdmmc(pins, prec, clocks);

    // Kart takili degilse veya henuz hazir degilse birkac kez dene.
    //
    // HAL'in ornegi burada `loop`'ta sonsuza kadar bekliyor -- bizim icin
    // KABUL EDILEMEZ: kart takili olmadan da 1 kHz sinus uretebilmeliyiz.
    let mut ready = false;
    for _ in 0..INIT_RETRIES {
        if sd.init(Hertz::from_raw(SD_BUS_FREQ)).is_ok() {
            ready = true;
            break;
        }
        delay.delay_ms(50u32);
    }
    if !ready {
        return None;
    }

    // Blok cihazi -> FAT32 katmani.
    let mut fs = Controller::new(sd.sdmmc_block_device(), NoTimeSource);

    // VolumeIdx(0) = ilk bolum (partition). Normal formatlanmis bir SD
    // kartta tek bolum vardir ve o da budur.
    let mut volume = fs.get_volume(VolumeIdx(0)).ok()?;
    let root = fs.open_root_dir(&volume).ok()?;

    let mut file = fs
        .open_file_in_dir(&mut volume, &root, CONFIG_FILENAME, Mode::ReadOnly)
        .ok()?;

    // Dosya tampona sigmiyorsa PARCA okumaktansa tamamen vazgeciyoruz.
    //
    // Bu kontrol olmadan cok sinsi bir hata olusuyor: `read` tamponu doldurup
    // durur ve bize "512 bayt okudum" der. Ayarlar dosyanin sonunda (uzun bir
    // yorum blogunun ardinda) duruyorsa parser sadece yorumlari gorur, hicbir
    // anahtar bulamaz ve sessizce TUM varsayilanlari dondurur.
    //
    // Disaridan bakinca "SD okundu, demek ki dosyada bir sorun var" gibi
    // gorunur -- oysa SD kusursuz calisiyordur. Kotu tarafi, hatanin BASARI
    // gibi gorunmesi. O yuzden burada net bir sekilde vazgecip bilinen
    // fallback yoluna donuyoruz.
    if file.length() as usize > buf.len() {
        return None;
    }

    // `read` tampon dolana VEYA EOF'a kadar okur (bkz. embedded-sdmmc
    // `volume_mgr.rs`), yani yukaridaki kontrolden sonra tek cagri dosyanin
    // tamamini getirir.
    let n = fs.read(&volume, &mut file, buf).ok()?;

    // Kapatmayi denetliyoruz ama hatasini onemsemiyoruz: sadece okuduk,
    // yazacak bir sey yok. Zaten bu fonksiyondan cikinca `fs` (ve icindeki
    // SDMMC cevrebirimi) drop olacak -- SD karta bir daha ihtiyacimiz yok.
    let _ = fs.close_file(&volume, file);
    fs.close_dir(&volume, root);

    Some(n)
}
