# WeAct STM32H750VBT6 — Osiloskop XY Kalp Çizici

`no_std` Rust. İki DAC kanalını **XY modunda** kullanarak osiloskop ekranına
**kalp** (ve istersen elips, çember, sekiz, yıldız) çizer. Çizim tamamen
donanımda üretilir: **DAC dual mode + TIM6 trigger + DMA circular**.

CPU açılışta seçili şeklin nokta tablosunu (LUT) hesaplayıp DMA'yı kurar, sonra
buton/LED dışında sinyale hiç karışmaz. Kesmeler, gecikmeler veya başka işler
çizimi bozmaz.

| | |
|---|---|
| **PA4** (DAC1_OUT1) | yatay eksen — **X** |
| **PA5** (DAC1_OUT2) | dikey eksen — **Y** |
| Osiloskop modu | **XY** (CH1→X, CH2→Y, GND ortak) |
| Varsayılan şekil | **Kalp** ❤️ |
| Şekiller | ellipse · circle · figure8 · **heart** · star · rose · butterfly · spiral · gear · elsan |
| Şekil değiştir | **K1 butonu** (PC13) |
| LUT | 1024 nokta, kanal başına 12-bit (0–4095) |
| Çizim hızı | `frame_hz` (varsayılan 60 fps) |
| Flash kullanımı | ~38 KB / 128 KB |
| Flashleme | **USB DFU** (ST-Link gerekmez) |

Osiloskobu **XY moduna** al, prob'ları PA4 (X) ve PA5 (Y)'ye bağla, GND ortak.
Ekranda kalp belirir.

---

## ⚡ Bilgisayarın yanında değil mi? (DFU ile flashleme)

Bu projeyi flashlemek için **Rust'ı kurmana veya derlemene gerek yok.** Hazır
firmware iki yoldan gelir:

**A) Repodaki hazır dosya** — `firmware/heart.bin` (ve `firmware/heart.hex`)
repoya commit'li. GitHub'dan doğrudan indir.

**B) GitHub Actions çıktısı** — her push'ta bulut firmware'i yeniden derler:
repo → **Actions** sekmesi → en son **firmware** çalışması → **Artifacts** →
`heart-firmware` (içinde `heart.bin` + `heart.hex`). `v1.0` gibi bir etiket
push edilirse dosyalar bir **Release**'e de eklenir.

### Karta yazma (STM32CubeProgrammer, DFU USB)

1. Board'u **DFU moduna al**:
   - **BOOT0** butonunu **basılı tut** (veya BOOT0 jumper'ını `1`'e al)
   - Basılı tutarken **NRST**'ye bas ve bırak
   - **BOOT0**'ı bırak
   - Aygıt Yöneticisi'nde **"STM32 Bootloader"** (`VID_0483&PID_DF11`) görünür
2. **STM32CubeProgrammer**'ı aç → sağ üstten bağlantı tipini **USB** seç →
   **Connect**.
3. **Open file** ile indirdiğin dosyayı seç:
   - **`heart.hex`** → adres dosyanın içinde gömülü, sadece **Download**'a bas.
   - veya **`heart.bin`** → **Download** ekranında başlangıç adresini
     **`0x08000000`** gir, sonra **Start Programming**.
4. **Yazınca BOOT0'ı `0`'a geri al**, board'a reset at. Yoksa reset yine
   bootloader'a düşer ve çizim başlamaz.

> `heart.bin` mi `heart.hex` mi? İkisi de aynı firmware. `.hex` başlangıç
> adresini içinde taşıdığı için CubeProgrammer'da adres yazmana gerek kalmaz —
> hata yapma ihtimalini azaltır. `dfu-util` kullanıyorsan `.bin` iste:
> `dfu-util -a 0 -s 0x08000000:leave -D heart.bin`

---

## Bilgisayarın yanındaysa: derle ve flashle

### Kurulum (tek seferlik)

```powershell
rustup target add thumbv7em-none-eabihf   # Cortex-M7F hedefi
rustup component add llvm-tools            # rust-objcopy: ELF -> .bin
cargo install cargo-binutils
```

DFU yükleyici (biri yeterli): **STM32CubeProgrammer** (ST resmi) veya
`dfu-util` (`choco install dfu-util`). İkisi de STM32'nin ROM'undaki USB DFU
bootloader'ı ile konuşur; ST-Link gerekmez.

### Tek komut

Board'u DFU moduna alıp (yukarıdaki 1. adım):

```powershell
cargo run --release
```

`.cargo/config.toml`'daki `runner` bunu `flash.ps1`'e yönlendirir; betik ELF'i
`.bin`'e çevirip DFU ile yükler.

### Elle

```powershell
cargo build --release
rust-objcopy -O binary target\thumbv7em-none-eabihf\release\sinus heart.bin
# STM32CubeProgrammer:
& "$env:ProgramFiles\STMicroelectronics\STM32Cube\STM32CubeProgrammer\bin\STM32_Programmer_CLI.exe" `
  -c port=usb1 -w heart.bin 0x08000000 -v -g 0x08000000
# veya dfu-util:
dfu-util -a 0 -s 0x08000000:leave -D heart.bin
```

---

## Şekiller ve ayarlar

Başlangıç şekli ve çizim parametreleri iki kaynaktan gelir; SD kart yoksa
gömülü varsayılan kullanılır. **Her iki durumda da varsayılan KALP'tir.**

| Anahtar | Aralık | Anlamı |
|---|---|---|
| `shape` | ellipse·circle·figure8·**heart**·star·rose·butterfly·spiral | Başlangıç şekli (Türkçe adlar da geçer: gul, kelebek, spiral…) |
| `amplitude` | 0.0 – 1.0 | Çizim boyutu (tam ölçeğin oranı) |
| `offset` | 0.0 – 1.0 | Çizim merkezi (0.5 ≈ ekran ortası, ~1.65 V) |
| `frame_hz` | 20 – 200 | Saniyedeki çizim tekrarı (titremesin diye ≥ 50) |

- **Gömülü varsayılan:** `src/main.rs` → `FALLBACK_CONFIG` (şu an `heart`).
- **SD karttan:** FAT32 kartın köküne `SINUS.CFG` (repoda örneği var). Dosya
  adı 8.3 formatında olmalı; `embedded-sdmmc` uzun ad tanımaz. Kart/dosya
  yoksa sessizce gömülü varsayılana döner — bir çizici config yüzünden
  açılmamazlık etmemeli.
- **Çalışırken şekil değiştir:** **K1** (PC13) her basışta sıradaki şekle
  geçer; User LED (PE3) kaçıncı şekilde olduğunu yanıp sönerek gösterir.

### SD bağlantısı (WeAct board — SDMMC1, 4-bit)

| Sinyal | Pin | AF |
|---|---|---|
| CLK | PC12 | 12 |
| CMD | PD2 | 12 |
| D0–D3 | PC8, PC9, PC10, PC11 | 12 |

---

## Nasıl çalışıyor?

```
  TIM6 ---TRGO---+--> DAC kanal 1 (TEN1=1, TSEL1=5) --> PA4 (X)
                 |         |
                 |         +--> DMA isteği (DMAEN1=1)
                 |                    |
                 |              DMA1 Stream0 (circular, 32-bit)
                 |                    |
                 |                    v
                 |              DHR12RD'ye tek 32-bit yazma
                 |              (iki kanalı BİRDEN yükler)
                 |
                 +--> DAC kanal 2 (TEN2=1, TSEL2=5) --> PA5 (Y)
```

LUT'taki her nokta bir 32-bit kelime: alt 16 bit = X (kanal 1), üst 16 bit =
Y (kanal 2). `DHR12RD` ("Dual Holding Register, 12-bit Right-aligned") tek bir
32-bit yazmayla her iki kanalı **aynı anda** yükler:

```
 bit 31    28 27                16 15    12 11                 0
 +-----------+--------------------+---------+-------------------+
 |  ayrılmış |  DACC2DHR (Y=PA5)  | ayrılmış| DACC1DHR (X=PA4)  |
 +-----------+--------------------+---------+-------------------+
```

Aynı TRGO iki kanalı aynı anda tetikler; DMA tek transferde ikisinin verisini
yazar → X ve Y hiç ayrışmaz, çizim kaymaz. `DMAEN2` **bilinçli kapalı**: iki
kanalda da DMA açık olsaydı her tetikte iki istek üretilir, DMA LUT'ta iki adım
ilerleyip şekli bozardı.

### Donanım seçimleri

- **PA4 / PA5** — seçim değil, silikon gerçeği. H750'de DAC çıkışları başka
  pine götürülemez.
- **TIM6** — "basic timer": tek işi periyodik sayıp TRGO üretmek. Pin çıkışı
  yok → hiçbir header pini harcanmaz.
- **DMA1 Stream 0** — H7'de DMAMUX var; keyfi ama serbest seçim.
- **sys_ck 400 MHz** → hclk 200 → pclk1 100 → **timx_ker_ck 200 MHz**.

---

## ⚠️ H7'nin en büyük tuzağı: DTCM ve DMA

**DMA1/DMA2, DTCM RAM'e (`0x20000000`) erişemez.** DTCM çekirdeğe özel bir
bus ile bağlıdır; DMA ise AHB üzerinden çalışır. DTCM'deki bir tampona DMA
kurarsan kod sorunsuz derlenir ama çalışma anında DAC'tan bir şey çıkmaz.

Bu projede LUT `memory.x`'teki `.axisram` bölümüne taşınıyor:

```rust
#[link_section = ".axisram"]
static mut LUT: MaybeUninit<[u32; POINTS]> = MaybeUninit::uninit();
```

Doğrulama (derleme sonrası):

```
rust-nm --print-size target\thumbv7em-none-eabihf\release\sinus | Select-String LUT
# 24000000 ... LUT   <- 0x24... = AXI SRAM, DMA erişebilir  ✓
# 0x20...  görürsen DMA çalışmaz.
```

**D-cache** bilinçli olarak açılmıyor — açılsaydı CPU'nun yazdığı LUT cache'te
kalır, DMA RAM'den bayat veri okurdu.

---

## Dosya düzeni

| Dosya | İçerik |
|---|---|
| `src/main.rs` | Clock/GPIO/DAC/TIM6/DMA kurulumu, buton/LED döngüsü |
| `src/waveform.rs` | LUT üretimi (şekil → X/Y noktaları), `DHR12RD` paketleme |
| `src/shapes.rs` | Şekil kataloğu (kalp, elips, çember, sekiz, yıldız, gül, kelebek, spiral, dişli, ELSAN) |
| `src/text_elsan.rs` | "ELSAN" yazısının önceden hesaplanmış nokta tablosu |
| `tools/gen_text.py` | İstediğin kelimeyi vektör yazıya çevirip tablo üretir |
| `src/config.rs` | `Config` + `key=value` parser (varsayılan: kalp) |
| `src/sdcard.rs` | SDMMC1 + FAT32, `SINUS.CFG` okuma |
| `src/dac_dma.rs` | Dual mode, TSEL/TEN/DMAEN, TIM6 TRGO, DMA hedefi, start/stop |
| `memory.x` | H750 bellek haritası + `.axisram` bölümü |
| `build.rs` | `memory.x`'i linker arama yoluna koyar |
| `.cargo/config.toml` | Hedef (thumbv7em), linker, DFU runner |
| `.github/workflows/firmware.yml` | Bulutta derleyip `.bin`/`.hex` üretir |
| `firmware/heart.bin` · `firmware/heart.hex` | Hazır, DFU ile yazılabilir firmware |
| `flash.ps1` | ELF → .bin → DFU (yerel Windows kullanımı) |
| `SINUS.CFG` | SD karta konacak örnek config |

---

## Ölçüm / sorun giderme

Osiloskop **XY modunda**, PA4→X, PA5→Y, GND ortak:

- Ekranda kalp. Amplitude 0.85 → her eksen ~0.25 V – ~3.05 V arası salınır.
- Şekil bozuk/kaymışsa: prob'lar doğru eksende mi (PA4=X, PA5=Y)?
- Ekranda hiçbir şey yoksa sırayla:
  - `LUT` adresi `0x24...` mı (DTCM'ye düşmüş olabilir)?
  - BOOT0 jumper'ı `0`'a geri alındı mı?
  - DFU `:leave`/`-g` ile çıktı mı, yoksa reset at?
