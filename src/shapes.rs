//! Hazir sekil katalogu (osiloskop XY vektor cizimi icin).
//!
//! Her sekil, parametrik olarak `n` noktaya ornekleniyor. Nokta i'nin
//! koordinati normalize edilmis [-1, 1] araliginda dondurulur; DAC koduna
//! cevirme isi `waveform.rs`'de yapiliyor.
//!
//! Sekiller arasinda K1 butonu (PC13) ile gecilir, baslangic sekli SD
//! karttaki `SINUS.CFG` -> `shape = ...` satiriyla secilir.

use core::f32::consts::{PI, TAU};
use libm::{cosf, expf, fabsf, sinf};

/// Hazir sekiller. `#[repr(u8)]` sart degil ama enum'u kucuk tutar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Iki sinus, 60 derece faz farki -> egik elips. (Projenin ilk hali.)
    Ellipse,
    /// 90 derece -> tam cember.
    Circle,
    /// 1:2 frekans orani -> yatik sekiz (Lissajous).
    Figure8,
    /// Parametrik kalp.
    Heart,
    /// Bes koseli yildiz.
    Star,
    /// Bes yapraklı gul/cicek (rhodonea egrisi, r = cos(5θ)).
    Rose,
    /// Fay kelebek egrisi (Temple H. Fay, 1989).
    Butterfly,
    /// Ice ve disa donen simetrik spiral.
    Spiral,
}

impl Shape {
    /// Butonla donulen sira. Yeni sekil eklemek istersen sadece buraya ekle;
    /// `next`, `index` ve buton dongusu otomatik uyum saglar.
    pub const ALL: [Shape; 8] = [
        Shape::Ellipse,
        Shape::Circle,
        Shape::Figure8,
        Shape::Heart,
        Shape::Star,
        Shape::Rose,
        Shape::Butterfly,
        Shape::Spiral,
    ];

    /// Siradaki sekil (sona gelince basa sarar). Buton her basista bunu cagirir.
    pub fn next(self) -> Shape {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    /// `ALL` icindeki sirasi (0-tabanli). LED ile "kacinci sekil" gostermek icin.
    pub fn index(self) -> usize {
        // `position` bir Option doner; sekil her zaman ALL'da oldugu icin
        // unwrap_or(0) sadece derleyiciyi mutlu etmek icin.
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    /// `SINUS.CFG`'deki `shape = ...` degerini enum'a cevir. Turkce/Ingilizce
    /// iki isim de kabul; taninmazsa `None` -> cagiran varsayilani korur.
    pub fn from_name(s: &str) -> Option<Shape> {
        match s {
            "ellipse" | "elips" => Some(Shape::Ellipse),
            "circle" | "cember" => Some(Shape::Circle),
            "figure8" | "sekiz" => Some(Shape::Figure8),
            "heart" | "kalp" => Some(Shape::Heart),
            "star" | "yildiz" => Some(Shape::Star),
            "rose" | "gul" | "cicek" | "flower" => Some(Shape::Rose),
            "butterfly" | "kelebek" => Some(Shape::Butterfly),
            "spiral" => Some(Shape::Spiral),
            _ => None,
        }
    }

    /// Sekildeki i. nokta (0..n), normalize [-1, 1]. y yukari pozitif.
    pub fn sample(self, i: usize, n: usize) -> (f32, f32) {
        let t = i as f32 / n as f32; // 0..1, tam bir tur
        let a = TAU * t;
        match self {
            // 60 derece = PI/3 radyan.
            Shape::Ellipse => (sinf(a), sinf(a + PI / 3.0)),
            Shape::Circle => (sinf(a), cosf(a)),
            Shape::Figure8 => (sinf(a), sinf(2.0 * a)),
            Shape::Heart => {
                let s = sinf(a);
                let x = 16.0 * s * s * s;
                let y = 13.0 * cosf(a) - 5.0 * cosf(2.0 * a) - 2.0 * cosf(3.0 * a) - cosf(4.0 * a);
                // 17'ye bolerek kabaca [-1, 1]'e sigdiriyoruz.
                (x / 17.0, y / 17.0)
            }
            Shape::Star => star(t),

            // Gul/cicek: rhodonea r = cos(k·θ). k=5 (tek sayi) -> 5 yaprak.
            // r [-1, 1] arasinda, (x, y) birim cember icinde kaliyor; ekstra
            // olceklemeye gerek yok. Tek sayi k'de egri bir turda iki kez
            // cizilir (yapraklar birebir ust uste), gorsel olarak sorun degil.
            Shape::Rose => {
                let r = cosf(5.0 * a);
                (r * cosf(a), r * sinf(a))
            }

            // Fay kelebek egrisi. Parametre bir tam turda 0..24π gezer (t/12
            // teriminin periyodu 24π); boylece nokta baslangica tam doner ve
            // dongu KAPALI olur -- geri donus (retrace) cizgisi olmaz.
            //
            // Ham egrinin en genis noktasi ~3.93; 4.1'e bolerek ekrana tam
            // sigdiriyoruz (kirpilmadan, kenarda kucuk pay birakarak).
            Shape::Butterfly => {
                let u = TAU * 12.0 * t; // 0..24π
                let s12 = sinf(u / 12.0);
                let s5 = s12 * s12 * s12 * s12 * s12; // sin^5(u/12), powf'suz
                let rad = expf(cosf(u)) - 2.0 * cosf(4.0 * u) - s5;
                (sinf(u) * rad / 4.1, cosf(u) * rad / 4.1)
            }

            // Spiral: yaricap 0->1->0 ucgen zarfi (t'nin ortasinda 1), aci ise
            // surekli artar. Boylece merkezden acilip yine merkezde kapanan
            // simetrik bir spiral olur; basi ve sonu ayni nokta (orijin) oldugu
            // icin dongu kapali, stray cizgi yok.
            Shape::Spiral => {
                let r = 1.0 - fabsf(2.0 * t - 1.0); // 0..1..0
                let ang = TAU * 5.0 * t; // 5 tur
                (r * cosf(ang), r * sinf(ang))
            }
        }
    }
}

/// Bes koseli yildizin cevresi boyunca t (0..1) konumundaki nokta.
///
/// 10 kose (dis yaricap 1, ic yaricap 0.42) sirayla diziliyor. Simetri
/// geregi 10 kenarin hepsi esit uzunlukta, dolayisiyla t'yi dogrudan
/// kenarlara bolmek esit aralikli (esit parlaklikli) bir cizim verir.
fn star(t: f32) -> (f32, f32) {
    let e = t * 10.0; // 0..10
    let seg = e as usize % 10;
    let f = e - (e as usize) as f32; // kenar icindeki oran 0..1

    let vertex = |k: usize| -> (f32, f32) {
        // Tepe noktasi yukarida (+PI/2) baslasin, her adim 36 derece.
        let ang = PI / 2.0 + PI * (k as f32) / 5.0;
        let rad = if k % 2 == 0 { 1.0 } else { 0.42 };
        (rad * cosf(ang), rad * sinf(ang))
    };

    let a = vertex(seg);
    let b = vertex((seg + 1) % 10);
    (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
}
