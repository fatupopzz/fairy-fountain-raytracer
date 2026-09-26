//! El cielo nocturno que se ve por encima de la fuente.
//!
//! Antes el fondo era un degrade de tres azules: correcto, pero un rayo que
//! se iba al cielo devolvia un color plano y la mitad de arriba del cuadro
//! se leia como una pared pintada. Ahora es un SKYBOX de verdad: una imagen
//! equirectangular (longitud en U, latitud en V) que se genera UNA vez al
//! arrancar, por codigo, y se muestrea con la direccion del rayo. Ningun
//! archivo: el cielo es parte del programa, igual que las texturas del
//! relieve.
//!
//! Tres capas, de atras hacia adelante:
//!   1. un degrade de indigo, mas claro cerca del horizonte;
//!   2. una nebulosa de ruido fractal en dos colores (magenta y teal, la
//!      paleta de la fuente), concentrada en una banda inclinada, como la
//!      Via Lactea;
//!   3. estrellas: un hash por texel decide si hay una, otro cuanto brilla
//!      y de que color, y las mas brillantes llevan un halo de 3 x 3.
//!
//! Y encima una luna: un disco con borde suave en una direccion fija, que
//! es lo que hace que el cielo tenga un punto de luz al que mirar y de
//! donde bajan los god rays.

use crate::texture::TextureImage;
use crate::vec3::{normalize, Vec3};
use raylib::prelude::Color;
use std::f32::consts::PI;

/// Resolucion de la imagen equirectangular. 1024 x 512 alcanza: el cielo es
/// blando, y a 400 x 300 de trazado cada estrella ocupa un pixel de todos
/// modos.
const ANCHO: usize = 1024;
const ALTO: usize = 512;

/// Hacia donde esta la luna (se normaliza al usarla): detras de la fuente
/// (-Z), un poco a la izquierda y BAJA, a unos 11 grados sobre el
/// horizonte. Tiene que ser baja: la camara mira un poco hacia abajo y el
/// borde de arriba del cuadro queda a unos 8 grados de elevacion, asi que
/// una luna alta no aparece nunca. A 11 grados asoma justo por encima del
/// borde de roca del fondo y del techo de la fuente.
pub const LUNA_DIRECCION: Vec3 = Vec3::new(-0.28, 0.19, -0.94);

/// Suma de los tres canales a partir de la cual un texel del cielo se
/// considera una estrella. La nebulosa mas encendida no pasa de 260 y el
/// degrade de fondo anda por 60; una estrella arranca en 380.
const TITILEO_UMBRAL: u32 = 330;

/// Radianes por segundo del titileo. Lento: 0.9 es un ciclo cada siete
/// segundos, y con la fase repartida por celda el cielo entero respira sin
/// que se vea un patron.
const TITILEO_VELOCIDAD: f32 = 0.9;

/// Radio angular de la luna, en radianes (~2.5 grados: el doble de la real,
/// que en un cuadro de 45 grados de campo seria un punto).
const LUNA_RADIO: f32 = 0.045;

pub struct Cielo {
    imagen: TextureImage,
    /// Cuanto giro el cielo alrededor del eje vertical, en radianes. La
    /// noche AVANZA: la luna y las estrellas cruzan el cielo a lo largo de
    /// la cancion. Lo pone `ajustar` en cada cuadro.
    giro: f32,
    /// El segundo en el que estamos: la fase del titileo de las estrellas.
    tiempo: f32,
    /// Cuanto amanecio, de 0 (noche cerrada) a 1 (el horizonte encendido).
    /// Sobre el final del tema el cielo se calienta desde abajo: un
    /// resplandor rosa y oro que sube desde el horizonte y apaga las
    /// estrellas mas bajas.
    amanecer: f32,
}

impl Cielo {
    /// Genera el cielo entero. Tarda unas decenas de milisegundos, una vez.
    pub fn generar() -> Cielo {
        let mut pixels = Vec::with_capacity(ANCHO * ALTO);

        // Dos campos de ruido para la nebulosa, con semillas distintas, para
        // que el magenta y el teal no caigan en las mismas nubes.
        let nube_a = campo(ANCHO, ALTO, 6, 5, 11);
        let nube_b = campo(ANCHO, ALTO, 9, 5, 23);

        let luna = normalize(&LUNA_DIRECCION);

        for y in 0..ALTO {
            // Latitud: v = 0 arriba (+Y), v = 1 abajo.
            let v = (y as f32 + 0.5) / ALTO as f32;
            let elevacion = (0.5 - v) * PI; // +pi/2 arriba, -pi/2 abajo

            for x in 0..ANCHO {
                let u = (x as f32 + 0.5) / ANCHO as f32;
                let azimut = (u - 0.5) * 2.0 * PI;

                // --- 1. El degrade base ---
                // Mas claro pegado al horizonte, oscuro hacia el cenit y
                // casi negro por debajo (que apenas se ve).
                let t = (elevacion / (PI / 2.0)).clamp(-1.0, 1.0);
                let (mut r, mut g, mut b) = if t >= 0.0 {
                    mezcla((26.0, 20.0, 58.0), (8.0, 6.0, 24.0), t.powf(0.6))
                } else {
                    mezcla((26.0, 20.0, 58.0), (6.0, 5.0, 16.0), (-t).powf(0.5))
                };

                // --- 2. La nebulosa ---
                // Una banda inclinada: el peso cae con la distancia a una
                // sinusoide en el azimut, como una Via Lactea que cruza el
                // cielo en diagonal.
                let centro_banda = 0.35 * (azimut * 1.0 + 0.8).sin() + 0.15;
                let distancia_banda = (elevacion - centro_banda).abs();
                let banda = (1.0 - distancia_banda / 0.55).clamp(0.0, 1.0).powf(1.5);

                let na = nube_a[y * ANCHO + x];
                let nb = nube_b[y * ANCHO + x];
                // Contraste: el ruido promedia 0.5, se quiere que las
                // nubes sean islas y no un velo parejo.
                let magenta = ((na - 0.45) * 2.6).clamp(0.0, 1.0) * banda;
                let teal = ((nb - 0.5) * 2.8).clamp(0.0, 1.0) * banda;

                r += magenta * 95.0 + teal * 10.0;
                g += magenta * 22.0 + teal * 70.0;
                b += magenta * 100.0 + teal * 90.0;

                // --- 3. Las estrellas ---
                // Un hash por texel: una estrella cada ~140 texeles, mas
                // densas dentro de la banda. El brillo va con una potencia
                // alta para que casi todas sean tenues y unas pocas quemen.
                let h = hash2(x as u32, y as u32);
                let umbral = 0.9915 - banda * 0.005;
                if h > umbral {
                    let brillo = hash2(x as u32 + 977, y as u32 + 131).powf(3.0);
                    let tono = hash2(x as u32 + 31, y as u32 + 7);
                    // Blancas, algunas rosas y algunas cyan.
                    let (sr, sg, sb) = if tono < 0.6 {
                        (1.0, 0.97, 0.92)
                    } else if tono < 0.8 {
                        (1.0, 0.75, 0.9)
                    } else {
                        (0.7, 0.95, 1.0)
                    };
                    let e = 120.0 + brillo * 135.0;
                    r += sr * e;
                    g += sg * e;
                    b += sb * e;
                }

                // --- 4. La luna ---
                // Distancia angular entre este texel y la direccion de la
                // luna. Adentro del radio, un disco casi blanco con un
                // borde suave; alrededor, un halo tenue de tres radios.
                let dir = direccion(azimut, elevacion);
                let cos_ang = dir.dot(&luna).clamp(-1.0, 1.0);
                let ang = cos_ang.acos();
                if ang < LUNA_RADIO * 4.0 {
                    let disco = suave(1.0 - (ang - LUNA_RADIO * 0.85) / (LUNA_RADIO * 0.15));
                    let halo = (1.0 - ang / (LUNA_RADIO * 4.0)).clamp(0.0, 1.0).powf(2.5) * 0.35;
                    let luz = disco + halo;
                    r += luz * 235.0;
                    g += luz * 225.0;
                    b += luz * 205.0;
                }

                pixels.push(Color::new(
                    r.clamp(0.0, 255.0) as u8,
                    g.clamp(0.0, 255.0) as u8,
                    b.clamp(0.0, 255.0) as u8,
                    255,
                ));
            }
        }

        Cielo {
            imagen: TextureImage {
                width: ANCHO,
                height: ALTO,
                pixels,
            },
            giro: 0.0,
            tiempo: 0.0,
            amanecer: 0.0,
        }
    }

    /// Deja el cielo en el estado de este cuadro: girado `giro` radianes y
    /// con `amanecer` de resplandor en el horizonte. Se llama una vez por
    /// cuadro, antes de trazar.
    pub fn ajustar(&mut self, giro: f32, amanecer: f32, tiempo: f32) {
        self.giro = giro;
        self.amanecer = amanecer.clamp(0.0, 1.0);
        self.tiempo = tiempo;
    }

    /// Si la direccion apunta a la luna o a su halo.
    fn cerca_de_la_luna(&self, d: &Vec3) -> bool {
        // `d` ya viene con el giro deshecho, asi que se compara contra la
        // luna en su posicion de origen.
        d.dot(&normalize(&LUNA_DIRECCION)) > (LUNA_RADIO * 5.0).cos()
    }

    /// Hacia donde esta la luna AHORA, con el giro del cielo aplicado.
    pub fn luna(&self) -> Vec3 {
        girar_y(&normalize(&LUNA_DIRECCION), self.giro)
    }

    /// El color del cielo en la direccion `d` (normalizada).
    pub fn color(&self, d: &Vec3) -> Color {
        // El giro del cielo se aplica al reves sobre la direccion: girar
        // el cielo `giro` a la derecha es lo mismo que mirar `giro` a la
        // izquierda en la imagen fija.
        let d = girar_y(d, -self.giro);

        // Longitud y latitud de la direccion, a UV equirectangulares.
        let u = 0.5 + d.z.atan2(d.x) / (2.0 * PI);
        let v = 0.5 - d.y.clamp(-1.0, 1.0).asin() / PI;
        let u = u.rem_euclid(1.0);
        let v = v.clamp(0.0, 1.0);
        let mut noche = self.imagen.muestrear(u, v);

        // EL TITILEO. El cielo esta horneado, asi que una estrella no puede
        // parpadear por su cuenta: lo que se hace es modular el brillo de
        // los texeles que YA son mucho mas claros que el fondo, que son las
        // estrellas y nada mas (la nebulosa no llega ni a la mitad del
        // umbral). La fase sale de un hash de la CELDA de ocho por ocho
        // texeles en la que cae el punto, no del texel: una estrella ocupa
        // varios texeles por el filtrado bilineal, y con una fase por texel
        // sus pedazos parpadearian desacompasados y se veria como ruido.
        //
        // La luna queda afuera por el corte de arriba: es lo unico tan
        // brillante como una estrella y ocupa demasiado como para que
        // parpadear le quede bien.
        let brillo = noche.r as u32 + noche.g as u32 + noche.b as u32;
        if brillo > TITILEO_UMBRAL && !self.cerca_de_la_luna(&d) {
            let celda_x = (u * ANCHO as f32 / 8.0) as u32;
            let celda_y = (v * ALTO as f32 / 8.0) as u32;
            let fase = hash2(celda_x + 601, celda_y + 197) * 6.283;
            // Entre el 65% y el 100%: parpadean, no se apagan.
            let f = 0.825 + (self.tiempo * TITILEO_VELOCIDAD + fase).sin() * 0.175;
            let canal = |c: u8| (c as f32 * f) as u8;
            noche = Color::new(canal(noche.r), canal(noche.g), canal(noche.b), 255);
        }

        if self.amanecer <= 0.0 {
            return noche;
        }

        // El amanecer, en DOS CAPAS, y hacen falta las dos.
        //
        // La de abajo es la franja del horizonte: un resplandor que sube
        // desde la linea del suelo y se apaga con la altura, oro pegado al
        // horizonte y rosa mas arriba.
        //
        // La de arriba es la BOVEDA ENTERA. Mientras el amanecer era un
        // detalle del ultimo cuarto del tema alcanzaba con la franja, pero
        // ahora la escena ABRE de dia y tiene que leerse como de dia, y un
        // cielo que esta rosa abajo y negro con estrellas arriba no es un
        // amanecer: es una noche con una luz rara en el borde. A las cinco
        // de la manana el cenit no es negro, es un azul profundo pero
        // CLARO, y las estrellas de arriba se apagan igual que las de
        // abajo, solo que mas tarde.
        //
        // Asi que primero toda la boveda se lleva hacia ese azul, y encima
        // va la franja. El orden importa: al reves, la boveda lavaria el
        // oro del horizonte.
        let elevacion = d.y.clamp(-1.0, 1.0).asin();

        // --- Capa 1: la boveda ---
        //
        // El azul del cenit al alba. Mezclado por `amanecer * 0.72`: aun a
        // pleno dia queda casi un tercio de la noche debajo, y eso es lo
        // que deja ver todavia alguna estrella grande y la nebulosa
        // fantasma. Un cielo de amanecer completamente liso se ve pintado.
        const CENIT: (f32, f32, f32) = (54.0, 62.0, 112.0);
        // Y mas claro cuanto mas bajo se mire, que es como es: el aire
        // dispersa mas cerca del horizonte.
        let bajo = (1.0 - (elevacion / 0.9).clamp(0.0, 1.0)).powf(1.6);
        let dome = self.amanecer * 0.72;
        let boveda = |base: u8, cenit: f32| -> f32 {
            let destino = cenit + (170.0 - cenit) * bajo * 0.55;
            base as f32 + (destino - base as f32) * dome
        };
        let (mut r, mut g, mut b) = (
            boveda(noche.r, CENIT.0),
            boveda(noche.g, CENIT.1),
            boveda(noche.b, CENIT.2),
        );

        // --- Capa 2: la franja del horizonte ---
        let altura = (1.0 - elevacion / 0.45).clamp(0.0, 1.0);
        let franja = altura * altura * self.amanecer;
        let oro = (1.0 - elevacion / 0.12).clamp(0.0, 1.0) * franja;

        let canal = |base: f32, rosa: f32, dorado: f32| {
            (base * (1.0 - franja * 0.55) + rosa * franja + dorado * oro).clamp(0.0, 255.0) as u8
        };
        r = canal(r, 205.0, 110.0) as f32;
        g = canal(g, 110.0, 75.0) as f32;
        b = canal(b, 150.0, 20.0) as f32;

        Color::new(r as u8, g as u8, b as u8, 255)
    }
}

/// Gira un vector alrededor del eje Y.
pub fn girar_y(v: &Vec3, angulo: f32) -> Vec3 {
    let (s, c) = angulo.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}

/// Direccion unitaria a partir de azimut (sobre XZ) y elevacion. Inversa del
/// mapeo de `Cielo::color`: azimut = atan2(z, x).
fn direccion(azimut: f32, elevacion: f32) -> Vec3 {
    let c = elevacion.cos();
    Vec3::new(c * azimut.cos(), elevacion.sin(), c * azimut.sin())
}

fn mezcla(a: (f32, f32, f32), b: (f32, f32, f32), t: f32) -> (f32, f32, f32) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn suave(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Numero repetible en [0, 1] a partir de dos enteros.
fn hash2(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
}

/// Ruido fractal de `ancho` x `alto` en [0, 1], que cierra en U (el cielo
/// da la vuelta) con `celdas` celdas a lo ancho en la primera octava.
fn campo(ancho: usize, alto: usize, celdas: usize, octavas: usize, semilla: u32) -> Vec<f32> {
    let mut campo = vec![0.0f32; ancho * alto];
    let mut peso = 1.0f32;
    let mut suma = 0.0f32;
    let mut cx = celdas.max(1);

    for octava in 0..octavas {
        let cy = (cx / 2).max(1);
        let sem = semilla.wrapping_mul(7919).wrapping_add(octava as u32 * 104_729);
        for y in 0..alto {
            for x in 0..ancho {
                let fx = x as f32 * cx as f32 / ancho as f32;
                let fy = y as f32 * cy as f32 / alto as f32;
                let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                let sx = tx * tx * (3.0 - 2.0 * tx);
                let sy = ty * ty * (3.0 - 2.0 * ty);

                let v = |gx: usize, gy: usize| hash2((gx % cx) as u32 + sem, gy as u32 + (sem >> 8));
                let arriba = v(x0, y0) + (v(x0 + 1, y0) - v(x0, y0)) * sx;
                let abajo = v(x0, y0 + 1) + (v(x0 + 1, y0 + 1) - v(x0, y0 + 1)) * sx;
                campo[y * ancho + x] += (arriba + (abajo - arriba) * sy) * peso;
            }
        }
        suma += peso;
        peso *= 0.5;
        cx *= 2;
    }

    for c in campo.iter_mut() {
        *c /= suma;
    }
    campo
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La luna esta donde dice `LUNA_DIRECCION`: mirando ahi el cielo es
    /// casi blanco, y mirando al lado opuesto es oscuro.
    #[test]
    fn la_luna_esta_donde_se_la_puso() {
        let cielo = Cielo::generar();
        let hacia = cielo.color(&normalize(&LUNA_DIRECCION));
        let contra = cielo.color(&normalize(&-LUNA_DIRECCION));
        assert!(hacia.r > 200 && hacia.g > 200);
        assert!(contra.r < 120 && contra.b < 160);
    }

    /// El horizonte es mas claro que el cenit.
    /// Con amanecer el horizonte se calienta (mas rojo que azul) y con el
    /// cielo girado la luna se corre.
    #[test]
    fn el_amanecer_calienta_y_el_giro_mueve_la_luna() {
        let mut cielo = Cielo::generar();
        let horizonte = direccion(1.0, 0.03);
        let noche = cielo.color(&horizonte);
        cielo.ajustar(0.0, 1.0, 0.0);
        let alba = cielo.color(&horizonte);
        assert!(alba.r > noche.r + 60 && alba.r > alba.b);

        cielo.ajustar(0.6, 0.0, 0.0);
        let luna = cielo.luna();
        assert!((luna - normalize(&LUNA_DIRECCION)).magnitude() > 0.3);
        // Mirando a donde esta la luna ahora, se la ve.
        let c = cielo.color(&luna);
        assert!(c.r > 200);
    }

    /// Las estrellas parpadean y la luna no.
    #[test]
    fn las_estrellas_titilan_y_la_luna_no() {
        let mut cielo = Cielo::generar();

        // Se busca una estrella barriendo el cielo.
        let mut estrella = None;
        for i in 0..4000 {
            let d = direccion(i as f32 * 0.37, (i as f32 * 0.11).sin() * 0.9);
            let c = cielo.color(&d);
            if c.r as u32 + c.g as u32 + c.b as u32 > TITILEO_UMBRAL + 60 && !cielo.cerca_de_la_luna(&d) {
                estrella = Some(d);
                break;
            }
        }
        let estrella = estrella.expect("el cielo tiene que tener estrellas");

        // A lo largo de un ciclo, su brillo cambia de verdad.
        let brillo = |c: &Cielo, d: &Vec3| {
            let x = c.color(d);
            x.r as i32 + x.g as i32 + x.b as i32
        };
        let mut min = i32::MAX;
        let mut max = i32::MIN;
        for paso in 0..24 {
            cielo.ajustar(0.0, 0.0, paso as f32 * 0.3);
            let b = brillo(&cielo, &estrella);
            min = min.min(b);
            max = max.max(b);
        }
        assert!(max - min > 30, "la estrella no titila: {min}..{max}");

        // La luna se queda quieta.
        let luna = normalize(&LUNA_DIRECCION);
        cielo.ajustar(0.0, 0.0, 0.0);
        let a = brillo(&cielo, &luna);
        cielo.ajustar(0.0, 0.0, 1.8);
        assert_eq!(a, brillo(&cielo, &luna));
    }

    #[test]
    fn el_horizonte_es_mas_claro() {
        let cielo = Cielo::generar();
        let mut horizonte = 0u32;
        let mut cenit = 0u32;
        for i in 0..32 {
            let a = i as f32 / 32.0 * 2.0 * PI;
            let h = cielo.color(&direccion(a, 0.02));
            horizonte += h.r as u32 + h.g as u32 + h.b as u32;
        }
        let c = cielo.color(&Vec3::new(0.0, 1.0, 0.0));
        cenit += c.r as u32 + c.g as u32 + c.b as u32;
        assert!(horizonte / 32 > cenit);
    }
}
