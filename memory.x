/* ==========================================================================
 * STM32H750VBT6 -- WeAct MiniSTM32H7xx board
 * ==========================================================================
 *
 * Bellek haritasi (RM0433 / DS12556). H750 "value line" parcasidir: cekirdek
 * ve RAM'i H743 ile ayni, ama dahili flash SADECE 128 KB (tek bank, Bank1).
 *
 *   FLASH   0x0800_0000  128K   Dahili flash. DFU bootloader buraya yazar.
 *   ITCM    0x0000_0000   64K   Komut TCM (burada kullanmiyoruz)
 *   DTCM    0x2000_0000  128K   Veri TCM -- 0 wait-state, CPU icin en hizli
 *   AXISRAM 0x2400_0000  512K   D1 domain, AXI bus
 *   SRAM1   0x3000_0000  128K   D2 domain
 *   SRAM2   0x3002_0000  128K   D2 domain
 *   SRAM3   0x3004_0000   32K   D2 domain
 *   SRAM4   0x3800_0000   64K   D3 domain
 *
 * !!! EN ONEMLI KISIM -- DTCM ve DMA !!!
 *
 * DTCM, Cortex-M7 cekirdegine "ozel" bir bus ile baglidir. DMA1/DMA2
 * kontrolcüleri AHB uzerinden calisir ve DTCM'ye ULASAMAZ. DTCM'de duran bir
 * tampona DMA kurarsaniz derleyici hic sikayet etmez, kod sorunsuz build olur,
 * ama calisma aninda DMA ya hicbir sey yapmaz ya da transfer error verir.
 * Bu, H7'de en sik yasanan ve en cok vakit kaybettiren tuzaktir.
 *
 * Cozum: varsayilan RAM'i (stack, .data, .bss) hizli olsun diye DTCM'de
 * tutuyoruz, ama DMA'nin okuyacagi sinus LUT'unu asagidaki `.axisram`
 * bolumune tasiyoruz. AXI SRAM D1 domain'dedir ve DMA1 (D2 domain) oraya
 * D2->D1 AHB koprusu uzerinden erisebilir. Bu ST'nin kendi orneklerinin de
 * kullandigi standart yoldur.
 * ========================================================================== */

MEMORY
{
  FLASH   (rx)  : ORIGIN = 0x08000000, LENGTH = 128K
  DTCM    (rwx) : ORIGIN = 0x20000000, LENGTH = 128K
  AXISRAM (rwx) : ORIGIN = 0x24000000, LENGTH = 512K
  SRAM1   (rwx) : ORIGIN = 0x30000000, LENGTH = 128K
  SRAM2   (rwx) : ORIGIN = 0x30020000, LENGTH = 128K
  SRAM3   (rwx) : ORIGIN = 0x30040000, LENGTH = 32K
  SRAM4   (rwx) : ORIGIN = 0x38000000, LENGTH = 64K
}

/* cortex-m-rt "RAM" adinda bir bolge bekler: .data, .bss ve stack oraya gider.
 * Stack tepesi otomatik olarak ORIGIN(RAM) + LENGTH(RAM) = 0x2002_0000 olur. */
REGION_ALIAS(RAM, DTCM);

/* DMA'nin erisebilecegi tampon icin ozel bolum.
 *
 * (NOLOAD) = "bu bolumu ELF'e veri olarak koyma, sadece adres ayir".
 * Sinus LUT'u calisma aninda libm::sinf ile hesaplandigi icin flash'ta bir
 * baslangic degeri tutmasina gerek yok. Boylece 128 KB'lik kucuk flash'imizi
 * bosa harcamiyoruz.
 *
 * DIKKAT: cortex-m-rt yalnizca RAM bolgesindeki .bss'i sifirlar. Buradaki
 * bellek acilista COP DOLUDUR (rastgele). Kod bu tamponu DMA baslamadan once
 * eksiksiz doldurmak zorundadir -- main.rs'de tam olarak bunu yapiyoruz. */
SECTIONS
{
  .axisram (NOLOAD) : ALIGN(4)
  {
    *(.axisram .axisram.*);
    . = ALIGN(4);
  } > AXISRAM
} INSERT AFTER .uninit;
