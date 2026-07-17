# WeAct STM32H750VBT6 — Çift Kanallı Sinüs Üreteci (60° faz farklı)

`no_std` Rust. İki sinüs sinyali, aralarında **60 derece** faz farkı, tamamen
donanımda üretilir: **DAC dual mode + TIM6 trigger + DMA circular**.

CPU açılışta LUT'u hesaplayıp DMA'yı kurar, sonra `wfi` ile uyur. Ana döngüde
tek satır sinyal kodu yoktur — kesmeler, gecikmeler veya başka işler dalgayı
etkilemez.

| | |
|---|---|
| **PA4** (DAC1_OUT1) | `sin(ωt)` |
| **PA5** (DAC1_OUT2) | `sin(ωt + 60°)` |
| Varsayılan frekans | 1 kHz (ayarlanabilir) |
| LUT | 128 örnek, 12-bit (0–4095) |
| Örnekleme hızı | `freq × 128` = 128 kSPS @ 1 kHz |
| Flash kullanımı | ~27 KB / 128 KB |
| Flashleme | USB DFU (**ST-Link gerekmez**) |

---

## 1. Kurulum (tek seferlik)

```powershell
# Cortex-M7 hedefi
rustup target add thumbv7em-none-eabihf

# ELF -> .bin çevirmek için (rust-objcopy)
rustup component add llvm-tools
cargo install cargo-binutils
```

**DFU yükleyici** — ikisinden biri yeterli, `flash.ps1` hangisi varsa onu bulur:

| Araç | Kurulum | Not |
|---|---|---|
| **STM32CubeProgrammer** | [st.com](https://www.st.com/en/development-tools/stm32cubeprog.html) | ST resmi. **Bu makinede kurulu, kullanılan bu.** |
| dfu-util | `choco install dfu-util` | Küçük, açık kaynak. **winget'te paketi yok.** |

> İkisi de aynı işi yapar: STM32'nin ROM'una gömülü USB DFU bootloader'ı ile
> konuşur. ST-Link hiçbirinde gerekmez.

---

## 2. Derle ve flashle

### Tek komut

```powershell
cargo run --release
```

`.cargo/config.toml`'daki `runner` bunu `flash.ps1`'e yönlendirir; betik
ELF'i `.bin`'e çevirir ve `dfu-util` ile yükler.

### Board'u DFU moduna alma

`cargo run` öncesi board DFU modunda olmalı:

1. **BOOT0** butonunu **basılı tut** (veya BOOT0 jumper'ını `1`'e al)
2. Basılı tutarken **NRST**'ye bas ve bırak
3. **BOOT0**'ı bırak
4. Aygıt Yöneticisi'nde **"STM32 Bootloader"** (`VID_0483&PID_DF11`) görünmeli

`flash.ps1` cihazı bulamazsa bu adımları zaten hatırlatır.

> **Yazdıktan sonra BOOT0'ı `0`'a geri al.** Yoksa bir sonraki reset yine
> bootloader'a düşer ve uygulaman çalışmaz.

### Adım adım (elle)

```powershell
# 1) Derle
cargo build --release

# 2) ELF -> ham .bin
rust-objcopy -O binary `
  target\thumbv7em-none-eabihf\release\sinus `
  target\thumbv7em-none-eabihf\release\sinus.bin

# 3a) STM32CubeProgrammer ile  (-v: doğrula, -g: yazınca çalıştır)
& "$env:ProgramFiles\STMicroelectronics\STM32Cube\STM32CubeProgrammer\bin\STM32_Programmer_CLI.exe" `
  -c port=usb1 -w target\thumbv7em-none-eabihf\release\sinus.bin 0x08000000 -v -g 0x08000000

# 3b) veya dfu-util ile  (:leave = yazınca çık ve çalıştır)
dfu-util -a 0 -s 0x08000000:leave -D target\thumbv7em-none-eabihf\release\sinus.bin
```

Sadece `.bin` üretmek için: `.\flash.ps1 <elf-yolu> -NoFlash`

Kartın DFU'da olup olmadığını kontrol etmek için:

```powershell
Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -match 'VID_0483&PID_DF11' }
# görünüyorsa  -> DFU modunda, yazmaya hazır
# görünmüyorsa -> uygulama çalışıyor (veya BOOT0 ayarı yanlış)
```

> **Neden `.bin`?** ELF bir *kap* dosyasıdır: bölüm başlıkları, sembol
> tablosu, debug bilgisi içerir. `dfu-util` ise flash'a byte byte ne
> yazacağını bilmek ister. `objcopy` sadece flash'a gidecek bölümleri
> (`.vector_table`, `.text`, `.rodata`) çıkarıp düz bir imaj üretir.

---

## 3. Frekans ve genlik ayarlama

Şu an ayarlar `src/main.rs` içindeki `FALLBACK_CONFIG` sabitinde:

```
freq_hz   = 1000.0
amplitude = 0.9
offset    = 0.5
```

| Anahtar | Aralık | Anlamı |
|---|---|---|
| `freq_hz` | 0.5 – 7800 | Çıkış frekansı (Hz) |
| `amplitude` | 0.0 – 1.0 | Tam ölçeğin oranı (1.0 ≈ 0–3.3 V tepeden tepeye) |
| `offset` | 0.0 – 1.0 | Salınım merkezi (0.5 ≈ 1.65 V) |

Değiştir → `cargo run --release`. Aralık dışı değerler kırpılır.

**Üst sınır neden 7800 Hz?** Örnekleme hızı `freq × 128` olduğu için 7800 Hz
≈ 1 MSPS eder. H750'nin DAC çıkış tamponu ~1.7 µs'de oturur, yani 1 MSPS
zaten pratik üst sınır. Daha yükseği için `LUT_LEN`'i küçült veya DAC'ı
tamponsuz kullan (`enable_unbuffered`).

---

## 4. SD karttan config okuma

**Çalışıyor.** Açılışta SD karttaki `SINUS.CFG` okunur; kart yoksa gömülü
`FALLBACK_CONFIG` devreye girer. Her iki yol da aynı parser'dan geçer.

### Kurulum

1. SD kartı **FAT32** formatla
2. Repo kökündeki `SINUS.CFG`'yi kartın **köküne** kopyala
3. Kartı yuvaya tak, board'a reset at

### Bağlantı (WeAct board — SDMMC1, 4-bit)

| Sinyal | Pin | AF |
|---|---|---|
| CLK | PC12 | 12 |
| CMD | PD2 | 12 |
| D0–D3 | PC8, PC9, PC10, PC11 | 12 |

### Dikkat edilecekler

- **Dosya adı 8.3 formatında olmalı.** `embedded-sdmmc` 0.5 uzun dosya adı
  desteklemez — `SINUS.CFG` olur, `sinus_config.txt` **bulunamaz**.
- **Config açılışta okunur.** Değiştirince reset at.
- Kart yok / FAT32 değil / dosya yok → sessizce varsayılanlara döner ve sinüs
  yine üretilir. Bir sinyal üreteci config dosyası yüzünden açılmamazlık
  etmemeli.

### Çalışırken değiştirmek istersen

- **Frekans:** DMA'ya hiç dokunmadan, sadece TIM6'nın ARR'sini değiştirerek
  anında ayarlanabilir (`dac_dma::configure_tim6`).
- **Genlik:** LUT'un yeniden üretilmesi gerekir → önce DMA'yı durdur.

### Neden burada DTCM sorunu yok?

LUT için "DMA DTCM'ye erişemez" diye özel bölüm açmıştık. SD tarafında böyle
bir dert yok: HAL'in SDMMC okuması **DMA kullanmıyor**, CPU FIFO register'ını
poll edip kopyalıyor (`sdmmc.rs` → `read_block`). Bu yüzden config tamponu
rahatça stack'te (DTCM) durabiliyor.

> Kural: **veriyi kim taşıyor?** CPU ise DTCM serbest, DMA ise değil.

---

## 5. Nasıl çalışıyor?

```
  TIM6 ---TRGO---+--> DAC kanal 1 (TEN1=1, TSEL1=5) --> PA4
                 |         |
                 |         +--> DMA isteği (DMAEN1=1)
                 |                    |
                 |              DMA1 Stream0 (circular, 32-bit)
                 |                    |
                 |                    v
                 |              DHR12RD'ye tek 32-bit yazma
                 |              (iki kanalı BİRDEN yükler)
                 |
                 +--> DAC kanal 2 (TEN2=1, TSEL2=5) --> PA5
```

**Drift neden imkânsız?** `DHR12RD` ("Dual Holding Register, 12-bit
Right-aligned") tek bir 32-bit yazmayla her iki kanalı yükler:

```
 bit 31    28 27                16 15    12 11                 0
 +-----------+--------------------+---------+-------------------+
 |  ayrılmış |  DACC2DHR (kanal2) | ayrılmış| DACC1DHR (kanal1) |
 +-----------+--------------------+---------+-------------------+
```

Aynı TRGO iki kanalı aynı anda tetikler; DMA tek transferde ikisinin de
verisini yazar. İki ayrı DMA / iki ayrı timer olsaydı ayrışabilirlerdi.

`DMAEN2` **bilinçli olarak kapalı**: iki kanalda da DMA açık olsaydı her
tetiklemede iki istek üretilir, DMA LUT'ta iki adım ilerleyip dalgayı bozardı.

### Donanım seçimlerinin gerekçesi

- **PA4 / PA5** — seçim değil, silikon gerçeği. H750'de DAC çıkışları başka
  pine götürülemez. HAL de bunu doğruluyor: `Pins<DAC1>` yalnızca
  `PA4<Analog>` / `PA5<Analog>` için implement edilmiş.
- **TIM6** — "basic timer": tek işi periyodik sayıp TRGO üretmek.
  Capture/compare kanalı, dolayısıyla pin çıkışı **yok** → hiçbir header pini
  harcanmaz. TIM2/3/4/5 PWM/encoder için serbest kalır. ST'nin DAC örnekleri
  de aynı sebeple TIM6 seçer.
- **DMA1 Stream 0** — H7'de DMAMUX var, herhangi bir istek herhangi bir
  stream'e yönlendirilebilir. Keyfi ama serbest seçim.
- **sys_ck 400 MHz** — H750 (rev V) 480 MHz'e çıkabilir ama 480 MHz VOS0
  ister ve ısınma/kararlılık açısından hassastır. Hız zaten gerekmiyor; iş
  DMA'da. 400 MHz → hclk 200 → pclk1 100 → **timx_ker_ck 200 MHz**.

---

## 6. ⚠️ H7'nin en büyük tuzağı: DTCM ve DMA

**DMA1/DMA2, DTCM RAM'e (`0x20000000`) erişemez.** DTCM çekirdeğe özel bir
bus ile bağlıdır; DMA ise AHB üzerinden çalışır.

DTCM'de duran bir tampona DMA kurarsan **kod sorunsuz derlenir**, ama
çalışma anında DAC'tan hiçbir şey çıkmaz. Bu, H7'de en çok vakit kaybettiren
hatadır.

Bu projede varsayılan RAM (stack, `.data`, `.bss`) hız için DTCM'de, ama sinüs
LUT'u `memory.x`'te tanımlı `.axisram` bölümüne taşınıyor:

```rust
#[link_section = ".axisram"]
static mut SINE_LUT: MaybeUninit<[u32; LUT_LEN]> = MaybeUninit::uninit();
```

Doğrulama (derleme sonrası):

```powershell
rust-nm --print-size target\thumbv7em-none-eabihf\release\sinus | Select-String SINE_LUT
# 24000000 00000200 b ...SINE_LUT     <- 0x24... = AXI SRAM, DMA erişebilir  ✓
```

`0x20...` görürsen DMA çalışmaz.

> `.axisram` bölümü `(NOLOAD)`: açılışta ne flash'tan yüklenir ne de
> sıfırlanır. Bu yüzden LUT `MaybeUninit` ve DMA başlamadan **önce** tamamen
> doldurulur. Ayrıca 512 baytlık tablo flash'ta yer kaplamaz.

**D-cache** bilinçli olarak açılmıyor — açılsaydı CPU'nun yazdığı LUT cache'te
kalır, DMA ise RAM'den bayat veri okurdu.

---

## 7. Dosya düzeni

| Dosya | İçerik |
|---|---|
| `src/main.rs` | Clock/GPIO/DAC/TIM6/DMA kurulumu ve başlatma sırası |
| `src/waveform.rs` | LUT üretimi (`libm::sinf`), `DHR12RD` paketleme |
| `src/config.rs` | `SineConfig` + `key=value` parser |
| `src/sdcard.rs` | SDMMC1 + FAT32, `SINUS.CFG` okuma |
| `src/dac_dma.rs` | Dual mode, TSEL/TEN/DMAEN, TIM6 TRGO, DMA hedefi |
| `memory.x` | H750 bellek haritası + `.axisram` bölümü |
| `.cargo/config.toml` | Hedef, linker, DFU runner |
| `flash.ps1` | ELF → .bin → dfu-util |
| `SINUS.CFG` | SD karta konacak örnek config |

---

## 8. `unsafe` kullanımı

Toplam üç yerde, hepsi gerekçelendirilmiş:

1. **`waveform.rs`** — ilklendirilmemiş `.axisram` belleğini doldurmak.
   `AtomicBool` ile tek sahiplik garantisi altında.
2. **`dac_dma.rs` → `configure_dual_mode`** — HAL dual mode'u sarmalamıyor.
   Tek `dac.cr.modify()` çağrısı; `tsel1/tsel2().bits()` unsafe çünkü PAC bu
   alanları enum'lamamış.
3. **`dac_dma.rs` → `TargetAddress` impl** — trait'in kendisi `unsafe`; HAL
   verdiğimiz adresi doğrulayamaz.

Geri kalan her şey HAL'in tip-güvenli API'siyle. `configure_tim6` içindeki
`psc/arr/mms/ug` yazmaları **unsafe değil** — PAC bu alanları
`FieldWriterSafe` / enum olarak üretmiş.

### PAC'ta bir tuzak: `TSEL` alan genişliği

ST'nin resmi CMSIS başlığı (`stm32h743xx.h`):

```c
#define DAC_CR_TSEL1_Pos   (2U)
#define DAC_CR_TSEL1_Msk   (0xFUL << DAC_CR_TSEL1_Pos)   // 0x0000003C -> 4 bit
```

ve `stm32h7xx_hal_dac.h`:

```c
#define DAC_TRIGGER_T6_TRGO  (DAC_CR_TSEL1_2 | DAC_CR_TSEL1_0 | DAC_CR_TEN1)
```

→ `TSEL = 0b0101 = 5` = TIM6 TRGO.

**Ama `stm32h7` PAC 0.15.1 bu alanı 3 bit olarak modelliyor** (ST'nin kendi
başlığındaki eski `TSEL1[2:0]` yorumundan gelen SVD hatası). Bizim için sorun
değil — 5 zaten 3 bite sığıyor ve doğru bitlere yazılıyor. Ama TIM15 (8) veya
LPTIM1 (11) gibi bir tetiğe geçersen PAC değeri sessizce maskeleyip **yanlış**
yazar; o durumda `CR`'yi ham `bits()` ile yazman gerekir.

---

## 9. Ölçüm

Osiloskopu PA4 ve PA5'e bağla (GND ortak):

- İki sinüs, ~1 kHz, ~0.15 V – ~3.15 V arası (amplitude 0.9)
- Aralarında 60° faz farkı → 1 kHz'de **~167 µs** gecikme
  (`60/360 × 1 ms`). PA5 önde (leading).
- Basamak yapısı: 128 adım/tur

Sinyal yoksa sırayla: `SINE_LUT` adresi `0x24...` mı? BOOT0 jumper'ı `0`'a
geri alındı mı? DFU `:leave` ile çıktı mı (yoksa reset at)?
