//! SD karttaki `SINUS.CFG` ayarlari ve ayristirici.
//!
//! Donanimdan tamamen bagimsiz: sadece metin -> `Config`. SDMMC surucusu
//! byte'lari getiriyor (bkz. `sdcard.rs`), gerisi burada.

use crate::shapes::Shape;

/// Cizim tekrar hizi (frame rate) sinirlari, Hz.
///
/// Alt sinir titremeyi (flicker) onler; ust sinir gereksiz yuksek ornekleme
/// hizini engeller. Timer hizi = frame_hz * POINTS oldugu icin ust sinir
/// pratikte DAC'i zorlamamak icin.
pub const MIN_FRAME_HZ: f32 = 20.0;
pub const MAX_FRAME_HZ: f32 = 200.0;

/// Cizim ayarlari.
#[derive(Clone, Copy)]
pub struct Config {
    /// Baslangicta cizilecek sekil (butonla degistirilebilir).
    pub shape: Shape,
    /// Genlik: cizimin boyutu, tam olcegin orani. 0.0..1.0.
    pub amplitude: f32,
    /// Offset: cizimin merkezi. 0.5 = ekran ortasi (~1.65 V).
    pub offset: f32,
    /// Saniyedeki cizim tekrari (frame rate). Titremesin diye >= ~50 iyi.
    pub frame_hz: f32,
}

impl Default for Config {
    /// SD yoksa / okunamazsa kullanilan degerler.
    ///
    /// Varsayilan sekil KALP: SD kart olmasa bile board acilir acilmaz
    /// osiloskop ekranina kalp cizer.
    fn default() -> Self {
        Self {
            shape: Shape::Heart,
            amplitude: 0.85,
            offset: 0.5,
            frame_hz: 60.0,
        }
    }
}

impl Config {
    /// Degerleri guvenli araliga cek (SD'den gelen veriye guvenme).
    pub fn clamped(self) -> Self {
        Self {
            shape: self.shape,
            amplitude: clamp_f32(self.amplitude, 0.0, 1.0),
            offset: clamp_f32(self.offset, 0.0, 1.0),
            frame_hz: clamp_f32(self.frame_hz, MIN_FRAME_HZ, MAX_FRAME_HZ),
        }
    }

    /// `anahtar = deger` formatindaki config metnini ayristir.
    ///
    /// Beklenen dosya (SD kartin kokune `SINUS.CFG`):
    ///
    /// ```text
    /// shape     = heart        # ellipse|circle|figure8|heart|star|rose|butterfly|spiral
    /// amplitude = 0.85
    /// offset    = 0.5
    /// frame_hz  = 60
    /// ```
    ///
    /// Taninmayan anahtar / bozuk satir sessizce atlanir; eksik alan
    /// varsayilaninda kalir. Boylece bozuk bir dosya board'u kilitlemez.
    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();

        for line in text.lines() {
            let line = match line.split('#').next() {
                Some(l) => l.trim(),
                None => continue,
            };
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());

            match key {
                // Sekil bir sayi degil, isim: kendi ayristiricisi var.
                "shape" => {
                    if let Some(s) = Shape::from_name(value) {
                        c.shape = s;
                    }
                }
                "amplitude" => {
                    if let Ok(x) = value.parse::<f32>() {
                        c.amplitude = x;
                    }
                }
                "offset" => {
                    if let Ok(x) = value.parse::<f32>() {
                        c.offset = x;
                    }
                }
                "frame_hz" => {
                    if let Ok(x) = value.parse::<f32>() {
                        c.frame_hz = x;
                    }
                }
                _ => {}
            }
        }

        c.clamped()
    }

    /// SD karttan okunmus ham byte'lardan config uret.
    pub fn parse_bytes(bytes: &[u8]) -> Self {
        match core::str::from_utf8(bytes) {
            Ok(text) => Self::parse(text),
            Err(_) => Self::default(),
        }
    }
}

/// `f32::clamp` `core`'da yok; kendimiz yaziyoruz. NaN girdi `min`'e duser.
fn clamp_f32(v: f32, min: f32, max: f32) -> f32 {
    if v > max {
        max
    } else if v > min {
        v
    } else {
        min
    }
}
