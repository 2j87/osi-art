# ===========================================================================
# WeAct STM32H750VBT6 -- USB DFU ile flashleme
#
# `cargo run --release` bu betigi calistirir (.cargo/config.toml -> runner).
# Cargo, derledigi ELF'in yolunu son arguman olarak ekler.
#
# Elle de calistirabilirsin:
#   .\flash.ps1 .\target\thumbv7em-none-eabihf\release\sinus
#
# ST-Link/probe-rs KULLANILMIYOR.
# ===========================================================================

param(
    # Cargo'nun uretip bize verdigi ELF dosyasi.
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$ElfPath,

    # Sadece .bin uret, DFU ile yukleme yapma.
    [switch]$NoFlash
)

$ErrorActionPreference = 'Stop'

# STM32 dahili flash'in baslangic adresi. DFU bootloader'a "bu ikiliyi nereye
# yazacagini" boyle soyluyoruz. 0x08000000 = memory.x'teki FLASH ORIGIN.
$FlashAddress = '0x08000000'

# ST'nin ROM'daki DFU bootloader'inin USB kimligi (tum STM32'lerde ayni).
$DfuVid = '0483:df11'

# --- 1) ELF -> ham .bin -----------------------------------------------------
#
# Neden gerekli? ELF; bolum basliklari, sembol tablosu ve debug bilgisi iceren
# bir KAP dosyasi. dfu-util ise flash'a byte byte ne yazacagini bilmek ister.
# objcopy, ELF'ten sadece gercekten flash'a gidecek bolumleri (.vector_table,
# .text, .rodata, .data'nin baslangic degerleri) cikarip duz bir imaj uretir.

if (-not (Test-Path $ElfPath)) {
    Write-Error "ELF bulunamadi: $ElfPath"
}

$BinPath = [System.IO.Path]::ChangeExtension($ElfPath, '.bin')

# `rust-objcopy` cargo-binutils ile gelir (bkz. README kurulum adimlari).
$objcopy = Get-Command rust-objcopy -ErrorAction SilentlyContinue
if (-not $objcopy) {
    Write-Error @'
rust-objcopy bulunamadi. Kurmak icin:
    rustup component add llvm-tools
    cargo install cargo-binutils
'@
}

Write-Host "[1/2] ELF -> BIN" -ForegroundColor Cyan
& $objcopy.Source -O binary $ElfPath $BinPath
if ($LASTEXITCODE -ne 0) { Write-Error "objcopy basarisiz (kod $LASTEXITCODE)" }

$size = (Get-Item $BinPath).Length
Write-Host ("      {0}  ({1:N0} bayt / 131072, %{2:N1} flash)" -f `
    $BinPath, $size, ($size / 131072 * 100))

# H750'nin dahili flash'i sadece 128 KB -- tasarsak DFU yazma hatasi verirdi.
if ($size -gt 131072) {
    Write-Error "Imaj 128 KB'lik dahili flash'a sigmiyor!"
}

if ($NoFlash) {
    Write-Host "-NoFlash verildi, DFU adimi atlaniyor." -ForegroundColor Yellow
    exit 0
}

# --- 2) DFU ile yaz ---------------------------------------------------------
#
# Iki yukleyiciden hangisi varsa onu kullaniyoruz. IKISI DE ayni seyi yapar:
# STM32'nin ROM'una gomulu USB DFU bootloader'i ile konusur. ST-Link YOK.
#
#   * dfu-util               -- kucuk, acik kaynak, capraz platform
#   * STM32_Programmer_CLI   -- ST'nin resmi araci (STM32CubeProgrammer ile gelir)
#
# Windows'ta dfu-util'i paketleyen bir winget paketi yok, bu yuzden
# CubeProgrammer zaten kuruluysa onu tercih etmek en az surtunmeli yol.

# Board DFU modunda mi? Yukleyiciden bagimsiz olarak USB'den kontrol ediyoruz.
$inDfu = $null -ne (Get-PnpDevice -PresentOnly -ErrorAction SilentlyContinue |
    Where-Object { $_.InstanceId -match 'VID_0483&PID_DF11' })

if (-not $inDfu) {
    Write-Host ""
    Write-Warning "DFU cihazi ($DfuVid) gorunmuyor. Board'u DFU moduna al:"
    Write-Host "  1. BOOT0 butonunu BASILI TUT (veya BOOT0 jumper'ini 1 yap)"
    Write-Host "  2. Basili tutarken NRST'ye bas ve birak"
    Write-Host "  3. BOOT0'i birak"
    Write-Host "  4. Bu komutu tekrar calistir"
    Write-Host ""
    Write-Error "DFU cihazi bulunamadi."
}

$dfuUtil = Get-Command dfu-util -ErrorAction SilentlyContinue
$cubeCli = @(
    "$env:ProgramFiles\STMicroelectronics\STM32Cube\STM32CubeProgrammer\bin\STM32_Programmer_CLI.exe",
    "${env:ProgramFiles(x86)}\STMicroelectronics\STM32Cube\STM32CubeProgrammer\bin\STM32_Programmer_CLI.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if ($dfuUtil) {
    Write-Host "[2/2] dfu-util ile yukleniyor..." -ForegroundColor Cyan

    # -a 0  : alternate setting 0 = "Internal Flash"
    # -s    : baslangic adresi. Sondaki `:leave` = yazma bitince DFU'dan cik ve
    #         uygulamayi hemen calistir.
    # -D    : download (PC -> cihaz)
    & $dfuUtil.Source -a 0 -s "${FlashAddress}:leave" -D $BinPath
    if ($LASTEXITCODE -ne 0) { Write-Error "dfu-util basarisiz (kod $LASTEXITCODE)" }
}
elseif ($cubeCli) {
    Write-Host "[2/2] STM32_Programmer_CLI ile yukleniyor..." -ForegroundColor Cyan

    # -c port=usb1 : ilk USB DFU cihazina baglan
    # -w <dosya> <adres> : ham .bin'i verilen adrese yaz
    # -v : yazdiktan sonra geri okuyup dogrula
    # -g <adres> : yazma bitince oradan calistir (dfu-util'deki `:leave` karsiligi)
    & $cubeCli -c port=usb1 -w $BinPath $FlashAddress -v -g $FlashAddress
    if ($LASTEXITCODE -ne 0) { Write-Error "STM32_Programmer_CLI basarisiz (kod $LASTEXITCODE)" }
}
else {
    Write-Error @'
Hicbir DFU yukleyicisi bulunamadi. Birini kur:

  * STM32CubeProgrammer (ST resmi, GUI + CLI):
      https://www.st.com/en/development-tools/stm32cubeprog.html

  * dfu-util (winget'te YOK):
      choco install dfu-util
      veya https://dfu-util.sourceforge.net -> indir, PATH'e ekle
'@
}

Write-Host ""
Write-Host "Tamam. PA4 ve PA5'i osiloskopla izleyebilirsin." -ForegroundColor Green
Write-Host "Not: BOOT0'i 0'a geri al, yoksa sonraki reset yine DFU'ya duser." -ForegroundColor Yellow
