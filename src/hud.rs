//! LO QUE SE DIBUJA ENCIMA DE LA IMAGEN TRAZADA, como en las cinematicas
//! de Ocarina of Time.
//!
//! Va despues de todo el post-procesado, en la GPU, con las primitivas de
//! raylib (circulos, triangulos, rectangulos y texto): no hay ni una imagen
//! del juego, todo se dibuja con codigo.
//!
//!   - En el titulo, "PRESS START" bajo el logo y debajo una inscripcion en
//!     ALFABETO HYLIANO (la fuente `hylian64.ttf`), como las del juego.
//!   - El CUADRO DE TEXTO del juego cuando Link levanta el Contenedor de
//!     Corazon (ver `corazon.rs`), con las letras apareciendo de a una y el
//!     nombre del objeto en rojo.
//!
//! Hubo tambien un HUD de partida (corazones, magia, rupias y el pentagrama
//! de la ocarina), pero la escena se lee mejor como una cinematica.

use crate::sync::SceneParams;
use raylib::prelude::*;

/// Cuando se ve el HUD: despues del titulo.
const HUD_ENTRA: f32 = 10.5;

/// LA LETRA DEL TEXTO: FOT-Chiaro, la de los textos de los juegos de
/// Nintendo, si esta en `resources/fonts/chiaro_b.otf` (es comercial: no va
/// en el repositorio). Si no esta, la letra pixelada de raylib.
pub type Letra<'a> = Option<&'a Font>;

/// El ancho de un texto en la letra del texto.
fn medir(letra: Letra, s: &str, tam: f32) -> f32 {
    match letra {
        Some(f) => f.measure_text(s, tam, tam * 0.04).x,
        None => {
            let c = std::ffi::CString::new(s).unwrap_or_default();
            unsafe { raylib::ffi::MeasureText(c.as_ptr(), tam as i32) as f32 }
        }
    }
}

/// Escribe en la letra del texto.
fn escribir<D: RaylibDraw>(d: &mut D, letra: Letra, s: &str, x: f32, y: f32, tam: f32, color: Color) {
    match letra {
        Some(f) => d.draw_text_ex(f, s, Vector2::new(x, y), tam, tam * 0.04, color),
        None => d.draw_text(s, x as i32, y as i32, tam as i32, color),
    }
}

fn suave(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn con_alfa(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, (c.a as f32 * a.clamp(0.0, 1.0)) as u8)
}

/// Un triangulo relleno sin importar el orden de los vertices (raylib solo
/// dibuja los que vienen en un sentido; se mandan los dos).
fn triangulo<D: RaylibDraw>(d: &mut D, a: Vector2, b: Vector2, c: Vector2, color: Color) {
    d.draw_triangle(a, b, c, color);
    d.draw_triangle(a, c, b, color);
}

/// Un corazon: dos circulos y un triangulo, con borde oscuro y un brillo.
fn corazon<D: RaylibDraw>(d: &mut D, x: f32, y: f32, r: f32, relleno: Color, alfa: f32) {
    let forma = |d: &mut D, r: f32, color: Color| {
        d.draw_circle_v(Vector2::new(x - r * 0.5, y - r * 0.15), r * 0.55, color);
        d.draw_circle_v(Vector2::new(x + r * 0.5, y - r * 0.15), r * 0.55, color);
        triangulo(
            d,
            Vector2::new(x - r * 1.03, y + r * 0.02),
            Vector2::new(x + r * 1.03, y + r * 0.02),
            Vector2::new(x, y + r * 1.05),
            color,
        );
    };
    forma(d, r * 1.18, con_alfa(Color::new(40, 10, 10, 230), alfa));
    forma(d, r, con_alfa(relleno, alfa));
    d.draw_circle_v(Vector2::new(x - r * 0.55, y - r * 0.32), r * 0.18, con_alfa(Color::new(255, 230, 230, 220), alfa));
}

/// Texto con sombra, como el del juego.
fn texto<D: RaylibDraw>(d: &mut D, letra: Letra, s: &str, x: f32, y: f32, tam: f32, color: Color, alfa: f32) {
    let sombra = (tam / 12.0).max(1.0);
    escribir(d, letra, s, x + sombra, y + sombra, tam, con_alfa(Color::new(0, 0, 0, 200), alfa));
    escribir(d, letra, s, x, y, tam, con_alfa(color, alfa));
}

/// "PRESS START" y la inscripcion hyliana del titulo. Ver `dibujar_titulo`
/// en main.rs: `alfa` es la del logo y `y` el borde de abajo del logo.
pub fn titulo<D: RaylibDraw>(d: &mut D, letra: Letra, hyliano: &Font, (ancho, alto): (f32, f32), y: f32, alfa: f32, tiempo: f32) {
    if alfa <= 0.01 {
        return;
    }
    let u = alto / 672.0;
    // Parpadea como en el juego: medio segundo prendido, un poco apagado.
    let parpadeo = if (tiempo * 1.6).fract() < 0.7 { 1.0 } else { 0.25 };
    let tam = 30.0 * u;
    let s = "PRESS START";
    let w = medir(letra, s, tam);
    let x = (ancho - w) * 0.5;
    // Borde claro alrededor, como las letras del titulo del juego.
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        escribir(d, letra, s, x + dx * 2.0 * u, y + dy * 2.0 * u, tam, con_alfa(Color::new(255, 220, 140, 200), alfa * parpadeo));
    }
    escribir(d, letra, s, x, y, tam, con_alfa(Color::new(215, 40, 30, 255), alfa * parpadeo));

    // "la gran fuente de las hadas", en hyliano.
    let ins = "la gran fuente de las hadas";
    let tam_h = 22.0 * u;
    let medida = hyliano.measure_text(ins, tam_h, 2.0 * u);
    d.draw_text_ex(
        hyliano,
        ins,
        Vector2::new((ancho - medida.x) * 0.5, y + tam * 1.5),
        tam_h,
        2.0 * u,
        con_alfa(Color::new(255, 215, 120, 230), alfa),
    );
}

/// Todo el HUD de la escena, en este instante.
pub fn dibujar<D: RaylibDraw>(d: &mut D, letra: Letra, (ancho, alto): (f32, f32), p: &SceneParams) {
    let t = p.tiempo;
    let u = alto / 672.0;
    let alfa = suave((t - HUD_ENTRA) / 1.5);
    if alfa <= 0.01 {
        return;
    }

    // ---- EL CUADRO DE TEXTO ----
    let inicio = crate::corazon::LLEGA + 0.6;
    let fin = crate::corazon::SE_VA - 0.3;
    let caja = suave((t - inicio) / 0.3) * (1.0 - suave((t - fin) / 0.4));
    if caja > 0.01 {
        let (cw, ch) = (ancho * 0.66, 92.0 * u);
        let (cx, cy) = ((ancho - cw) * 0.5, alto * 0.70);
        d.draw_rectangle_rounded(Rectangle::new(cx, cy, cw, ch), 0.25, 8, con_alfa(Color::new(0, 0, 0, 190), caja));
        // El icono del objeto, a la izquierda.
        corazon(d, cx + 44.0 * u, cy + ch * 0.48, 18.0 * u, Color::new(225, 30, 45, 255), caja);
        // Las letras aparecen de a una, como en el juego.
        let letras = ((t - inicio) * 32.0).max(0.0) as usize;
        let tam = 22.0 * u;
        let (tx, ty) = (cx + 84.0 * u, cy + 16.0 * u);
        let partes: [(&str, Color); 3] = [
            ("¡Conseguiste un ", Color::WHITE),
            ("Contenedor de Corazón", Color::new(255, 70, 60, 255)),
            ("!", Color::WHITE),
        ];
        let mut x = tx;
        let mut quedan = letras;
        for (s, color) in partes {
            let n = s.chars().count().min(quedan);
            quedan -= n;
            let visible: String = s.chars().take(n).collect();
            texto(d, letra, &visible, x, ty, tam, color, caja);
            x += medir(letra, s, tam);
        }
        let segunda = "Tu vida máxima aumentó en un corazón.";
        let n = segunda.chars().count().min(quedan);
        let visible: String = segunda.chars().take(n).collect();
        texto(d, letra, &visible, tx, ty + tam * 1.6, tam, Color::WHITE, caja);
        // Y cuando termino, el triangulito que parpadea abajo.
        if n == segunda.chars().count() && (t * 2.5).fract() < 0.6 {
            let (vx, vy) = (cx + cw - 26.0 * u, cy + ch - 18.0 * u);
            triangulo(
                d,
                Vector2::new(vx - 7.0 * u, vy - 5.0 * u),
                Vector2::new(vx + 7.0 * u, vy - 5.0 * u),
                Vector2::new(vx, vy + 6.0 * u),
                con_alfa(Color::new(80, 220, 80, 255), caja),
            );
        }
    }
}
