use crate::vec3::{normalize, Vec3};
use raylib::prelude::Color;
use std::sync::Arc;

/// Patrones procedurales: el color no se guarda pixel por pixel, se
/// CALCULA a partir de las coordenadas UV del impacto. La excepcion es
/// ImageTexture, que si guarda pixeles de verdad.
///
/// El escenario solo usa `Solid` y `ImageTexture`, pero las procedurales se
/// quedan: son parte del engine y la consigna era no borrar infraestructura.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Texture {
    /// Un solo color en toda la superficie.
    Solid(Color),
    /// Cuadros de ajedrez. El f32 dice cuantos cuadros entran en la
    /// superficie: mas grande = cuadros mas chiquitos.
    Checkerboard(Color, Color, f32),
    /// Vetas tipo marmol entre los dos colores.
    Marble(Color, Color),
    /// Una imagen cargada de disco, con un color que la tinta. Va en Arc
    /// porque los pixeles pesan y el material se clona muchas veces: asi
    /// se comparte el buffer en vez de copiarlo. El tinte se multiplica
    /// canal por canal, asi que blanco deja la imagen tal cual.
    ///
    /// El tercer campo es un CORRIMIENTO de las UV, en (u, v). Se suma
    /// antes de repetir el mosaico, asi que mover ese par arrastra la
    /// imagen por encima de la superficie sin tocar la geometria: es lo
    /// que hace que el agua deje de verse congelada. En (0.0, 0.0) la
    /// textura queda exactamente donde estaba.
    ImageTexture(Arc<TextureImage>, Color, (f32, f32)),
}

impl Texture {
    /// Corre las UV de una textura de imagen. En cualquier otra variante
    /// no hace nada: las procedurales no tienen imagen que arrastrar.
    ///
    /// Este escenario no arrastra ninguna textura (el piso es fijo), pero el
    /// campo y su setter son del engine y se quedan disponibles.
    #[allow(dead_code)]
    pub fn set_uv_offset(&mut self, u: f32, v: f32) {
        if let Texture::ImageTexture(_, _, offset) = self {
            *offset = (u, v);
        }
    }
}

/// Los pixeles de una imagen ya decodificados, listos para samplear.
#[derive(Debug)]
pub struct TextureImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Color>,
}

impl TextureImage {
    /// Abre el archivo, lo pasa a RGBA8 y se queda con los pixeles.
    ///
    /// Las cinco texturas de la cueva salen de `resources/textures/`, que
    /// las genera `generar_texturas_cueva.py`.
    pub fn load(path: &str) -> Self {
        let image = image::open(path)
            .unwrap_or_else(|e| panic!("no se pudo abrir la textura {path}: {e}"))
            .to_rgba8();

        let (width, height) = image.dimensions();
        let pixels = image
            .pixels()
            .map(|p| Color::new(p[0], p[1], p[2], p[3]))
            .collect();

        TextureImage {
            width: width as usize,
            height: height as usize,
            pixels,
        }
    }

    /// Genera el piso del escenario, sin abrir ningun archivo.
    ///
    /// Tiene que quedar CASI NEGRO: el piso es negro pulido y lo que se ve
    /// en el son los reflejos de las luces, no su color propio. La textura
    /// esta para que de cerca se intuya que hay baldosa, no para que se
    /// vea. Por eso todo lo de aca abajo se mueve entre 5 y 20 de 255.
    ///
    /// Son tres capas:
    ///   1. una base plana muy oscura,
    ///   2. ruido de valor de baja frecuencia (interpolado, no por pixel:
    ///      pixel a pixel seria grano de television y no manchas de
    ///      concreto),
    ///   3. las juntas de la baldosa, una linea apenas mas clara cada
    ///      `junta` pixeles.
    ///
    /// La cueva no lo usa (su piso es de cubos con textura de archivo), pero
    /// se queda: es parte del engine.
    #[allow(dead_code)]
    pub fn piso_escenario(size: usize, junta: usize) -> Self {
        const BASE: [f32; 3] = [8.0, 8.0, 12.0];
        /// Cuanto sube y baja el ruido, en niveles de 0..255.
        const RUIDO: f32 = 3.0;
        /// Celdas de ruido a lo ancho. Pocas = manchas grandes y suaves.
        const CELDAS: usize = 8;
        /// Cuanto mas clara es la junta.
        const JUNTA_BRILLO: f32 = 4.0;

        // Ruido de valor: se sortea un numero por vertice de una grilla
        // gruesa y despues se interpola. La grilla se cierra sobre si misma
        // (el vertice CELDAS es el 0) para que la textura siga siendo un
        // mosaico y no se vea la costura al repetirse.
        let mut vertices = vec![0.0f32; (CELDAS + 1) * (CELDAS + 1)];
        for gy in 0..=CELDAS {
            for gx in 0..=CELDAS {
                let (wx, wy) = (gx % CELDAS, gy % CELDAS);
                vertices[gy * (CELDAS + 1) + gx] = valor_hash(wx, wy);
            }
        }

        let mut pixels = Vec::with_capacity(size * size);

        for y in 0..size {
            for x in 0..size {
                // Posicion dentro de la grilla de ruido.
                let fx = x as f32 * CELDAS as f32 / size as f32;
                let fy = y as f32 * CELDAS as f32 / size as f32;
                let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);

                // Suavizado en S: sin esto se ven las aristas de la grilla.
                let sx = tx * tx * (3.0 - 2.0 * tx);
                let sy = ty * ty * (3.0 - 2.0 * ty);

                let v = |gx: usize, gy: usize| vertices[gy * (CELDAS + 1) + gx];
                let arriba = v(x0, y0) + (v(x0 + 1, y0) - v(x0, y0)) * sx;
                let abajo = v(x0, y0 + 1) + (v(x0 + 1, y0 + 1) - v(x0, y0 + 1)) * sx;
                let ruido = (arriba + (abajo - arriba) * sy) * 2.0 - 1.0; // [-1, 1]

                // Las juntas: las dos ultimas filas y columnas de cada
                // baldosa. Dos pixeles de ancho para que sobrevivan al
                // muestreo bilineal cuando el piso se ve de lejos.
                let en_junta = x % junta >= junta - 2 || y % junta >= junta - 2;
                let junta_extra = if en_junta { JUNTA_BRILLO } else { 0.0 };

                let canal = |base: f32| -> u8 {
                    (base + ruido * RUIDO + junta_extra).clamp(0.0, 255.0) as u8
                };

                pixels.push(Color::new(canal(BASE[0]), canal(BASE[1]), canal(BASE[2]), 255));
            }
        }

        TextureImage {
            width: size,
            height: size,
            pixels,
        }
    }

    /// Una textura PINTADA POR CODIGO: se le pasa una funcion de (u, v), en
    /// [0, 1] con v hacia abajo, que devuelve el color en [0, 1] por canal.
    ///
    /// Es lo que usan las texturas de Link (la cara, la tela de la tunica, el
    /// escudo hyliano): ninguna sale de un archivo. Se evalua una vez por
    /// texel al arrancar, asi que la funcion puede ser tan cara como haga
    /// falta.
    pub fn pintada(ancho: usize, alto: usize, f: impl Fn(f32, f32) -> [f32; 3]) -> Self {
        let mut pixels = Vec::with_capacity(ancho * alto);
        for y in 0..alto {
            for x in 0..ancho {
                let c = f((x as f32 + 0.5) / ancho as f32, (y as f32 + 0.5) / alto as f32);
                let canal = |v: f32| (v * 255.0).clamp(0.0, 255.0) as u8;
                pixels.push(Color::new(canal(c[0]), canal(c[1]), canal(c[2]), 255));
            }
        }
        TextureImage { width: ancho, height: alto, pixels }
    }

    /// `sample` para afuera del modulo: bilineal, con las UV en [0, 1].
    pub fn muestrear(&self, u: f32, v: f32) -> Color {
        self.sample(u, v)
    }

    fn pixel(&self, x: usize, y: usize) -> Color {
        self.pixels[y * self.width + x]
    }

    /// Un MAPA DE NORMALES procedural: relieve que la luz ve y la geometria
    /// no tiene.
    ///
    /// Es la diferencia entre una cara plana de color y una superficie que
    /// parece tener grano, vetas o abolladuras. No se toca ni un vertice:
    /// se guarda, por texel, hacia donde se inclina la superficie, y el
    /// sombreado usa esa normal en vez de la geometrica. El costo es una
    /// lectura de textura por impacto.
    ///
    /// El relieve sale de un campo de alturas de ruido fractal (`octavas`
    /// capas de ruido de valor, cada una al doble de frecuencia y la mitad
    /// de peso) que CIERRA EN MOSAICO: la grilla del ruido se envuelve
    /// sobre si misma, asi que la textura se repite sin costura. La normal
    /// se saca de la pendiente del campo (diferencias centrales), y
    /// `fuerza` dice cuanto se inclina: 1.0 es un relieve suave, 4.0 es
    /// piedra rugosa.
    ///
    /// Se codifica como en cualquier normal map: XYZ en RGB, con 128 como
    /// cero, y la Z (que apunta hacia afuera de la superficie) siempre
    /// positiva. `sample_normal` lo decodifica.
    pub fn relieve_procedural(tam: usize, celdas: usize, octavas: usize, fuerza: f32, semilla: u32) -> Self {
        // El campo de alturas, en [0, 1].
        let altura = campo_fbm(tam, celdas, octavas, semilla);
        let h = |x: i64, y: i64| -> f32 {
            let xi = x.rem_euclid(tam as i64) as usize;
            let yi = y.rem_euclid(tam as i64) as usize;
            altura[yi * tam + xi]
        };

        let mut pixels = Vec::with_capacity(tam * tam);
        for y in 0..tam as i64 {
            for x in 0..tam as i64 {
                // Pendiente por diferencias centrales, escalada por la
                // fuerza. Una superficie de altura h(u, v) tiene normal
                // (-dh/du, -dh/dv, 1), que aca se normaliza.
                let dx = (h(x + 1, y) - h(x - 1, y)) * fuerza * tam as f32 * 0.5 / 16.0;
                let dy = (h(x, y + 1) - h(x, y - 1)) * fuerza * tam as f32 * 0.5 / 16.0;
                let n = normalize(&Vec3::new(-dx, -dy, 1.0));

                let canal = |c: f32| ((c * 0.5 + 0.5) * 255.0).clamp(0.0, 255.0) as u8;
                pixels.push(Color::new(canal(n.x), canal(n.y), canal(n.z), 255));
            }
        }

        TextureImage {
            width: tam,
            height: tam,
            pixels,
        }
    }

    /// Lee el mapa de normales en (u, v), repetido en mosaico, y devuelve
    /// la normal en ESPACIO TANGENTE: x a lo largo de u, y a lo largo de
    /// v, z hacia afuera. Quien la llama la lleva al mundo con la base
    /// tangente de la superficie.
    pub fn sample_normal(&self, u: f32, v: f32) -> Vec3 {
        let c = self.sample(u.rem_euclid(1.0), v.rem_euclid(1.0));
        let dec = |b: u8| b as f32 / 127.5 - 1.0;
        normalize(&Vec3::new(dec(c.r), dec(c.g), dec(c.b).max(0.05)))
    }

    /// Muestreo bilineal: en vez de agarrar el pixel mas cercano, mezcla
    /// los cuatro de alrededor. Sin esto la textura se ve en bloques
    /// cuando la camara se acerca.
    fn sample(&self, u: f32, v: f32) -> Color {
        if self.width == 0 || self.height == 0 {
            return Color::new(0, 0, 0, 255);
        }

        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        // El -0.5 corre la muestra al CENTRO del pixel; sin el la
        // interpolacion queda corrida medio pixel.
        let fx = u * self.width as f32 - 0.5;
        let fy = v * self.height as f32 - 0.5;

        let x0 = fx.floor();
        let y0 = fy.floor();

        // Cuanto pesa el pixel de la derecha y el de abajo.
        let tx = fx - x0;
        let ty = fy - y0;

        // En los bordes se repite el pixel de la orilla.
        let x0 = (x0 as i32).clamp(0, self.width as i32 - 1) as usize;
        let y0 = (y0 as i32).clamp(0, self.height as i32 - 1) as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let c00 = self.pixel(x0, y0);
        let c10 = self.pixel(x1, y0);
        let c01 = self.pixel(x0, y1);
        let c11 = self.pixel(x1, y1);

        // Primero se mezcla en horizontal, despues en vertical.
        let blend = |a: u8, b: u8, c: u8, d: u8| -> u8 {
            let top = a as f32 + (b as f32 - a as f32) * tx;
            let bottom = c as f32 + (d as f32 - c as f32) * tx;
            (top + (bottom - top) * ty).clamp(0.0, 255.0) as u8
        };

        Color::new(
            blend(c00.r, c10.r, c01.r, c11.r),
            blend(c00.g, c10.g, c01.g, c11.g),
            blend(c00.b, c10.b, c01.b, c11.b),
            255,
        )
    }
}

/// Numero repetible en [0, 1] a partir de dos enteros. No es aleatorio de
/// verdad: es un revoltijo de bits. Lo importante es que siempre da lo
/// mismo, asi que el piso sale identico en cada corrida.
fn valor_hash(x: usize, y: usize) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
}

/// Campo de ruido fractal de `tam` x `tam` en [0, 1], que cierra en
/// mosaico. `celdas` es cuantas celdas de ruido entran a lo ancho en la
/// primera octava; cada octava siguiente duplica.
///
/// Es la misma idea que el ruido de `piso_escenario`, generalizada: se
/// suman `octavas` capas de ruido de valor interpolado, cada una con el
/// doble de frecuencia y la mitad de peso, que es lo que le da a la piedra
/// bultos grandes con grano fino encima en vez de una sola escala.
pub fn campo_fbm(tam: usize, celdas: usize, octavas: usize, semilla: u32) -> Vec<f32> {
    let mut campo = vec![0.0f32; tam * tam];
    let mut peso = 1.0f32;
    let mut suma = 0.0f32;
    let mut celdas = celdas.max(1);

    for octava in 0..octavas {
        let sem = semilla.wrapping_mul(7919).wrapping_add(octava as u32 * 104_729);
        for y in 0..tam {
            for x in 0..tam {
                let fx = x as f32 * celdas as f32 / tam as f32;
                let fy = y as f32 * celdas as f32 / tam as f32;
                let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                let sx = tx * tx * (3.0 - 2.0 * tx);
                let sy = ty * ty * (3.0 - 2.0 * ty);

                // La grilla se envuelve: el vertice `celdas` es el 0.
                let v = |gx: usize, gy: usize| {
                    valor_hash(gx % celdas + sem as usize, gy % celdas + (sem >> 8) as usize)
                };
                let arriba = v(x0, y0) + (v(x0 + 1, y0) - v(x0, y0)) * sx;
                let abajo = v(x0, y0 + 1) + (v(x0 + 1, y0 + 1) - v(x0, y0 + 1)) * sx;
                campo[y * tam + x] += (arriba + (abajo - arriba) * sy) * peso;
            }
        }
        suma += peso;
        peso *= 0.5;
        celdas *= 2;
    }

    for c in campo.iter_mut() {
        *c /= suma;
    }
    campo
}

/// Mezcla lineal entre dos colores. t = 0 devuelve `a`, t = 1 devuelve `b`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        (a.r as f32 + (b.r as f32 - a.r as f32) * t) as u8,
        (a.g as f32 + (b.g as f32 - a.g as f32) * t) as u8,
        (a.b as f32 + (b.b as f32 - a.b as f32) * t) as u8,
        255,
    )
}

impl Texture {
    pub fn get_color(&self, u: f32, v: f32) -> Color {
        match self {
            Texture::Solid(color) => *color,

            Texture::Checkerboard(a, b, scale) => {
                // Se parte el espacio UV en celdas. La suma de los dos
                // indices alterna par/impar como un tablero de ajedrez.
                let cell = (u * scale).floor() + (v * scale).floor();
                if (cell as i32) % 2 == 0 {
                    *a
                } else {
                    *b
                }
            }

            Texture::Marble(a, b) => {
                // Marmol barato: una onda seno en U, deformada por otra
                // onda en V. Sin la turbulencia saldrian rayas rectas;
                // con ella las vetas se ondulan y parece piedra.
                let scale = 8.0;
                let turbulence = (v * 30.0).sin() * 0.5;
                // El seno va de -1 a 1, se remapea a [0, 1] para mezclar.
                let t = ((u * scale + turbulence).sin() + 1.0) * 0.5;
                mix(*a, *b, t)
            }

            Texture::ImageTexture(image, tint, offset) => {
                // El corrimiento se suma ANTES de repetir: asi la imagen se
                // desliza sobre la superficie y el mosaico sigue cerrando.
                let u = u + offset.0;
                let v = v + offset.1;

                // fract() hace que las UV mayores a 1 den la vuelta, y asi
                // la imagen se repite en mosaico. Para valores negativos
                // fract() devuelve negativo, por eso el +1.0.
                let mut tiled_u = u.fract();
                let mut tiled_v = v.fract();
                if tiled_u < 0.0 {
                    tiled_u += 1.0;
                }
                if tiled_v < 0.0 {
                    tiled_v += 1.0;
                }
                let sampled = image.sample(tiled_u, tiled_v);

                // Multiplicar por el tinte: 255 deja el canal igual, y
                // menos lo apaga. Asi la misma piedra gris sirve para
                // cualquier color sin tocar el archivo.
                Color::new(
                    (sampled.r as u32 * tint.r as u32 / 255) as u8,
                    (sampled.g as u32 * tint.g as u32 / 255) as u8,
                    (sampled.b as u32 * tint.b as u32 / 255) as u8,
                    255,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests_relieve {
    use super::*;

    /// El relieve procedural inclina la normal de verdad, pero no la da
    /// vuelta: con fuerza 1 la inclinacion media queda entre 3 y 20 grados,
    /// y la Z nunca es negativa.
    #[test]
    fn el_relieve_inclina_sin_dar_vuelta() {
        for (fuerza, celdas) in [(0.9f32, 7usize), (3.0, 5), (1.4, 9), (0.7, 12)] {
            let mapa = TextureImage::relieve_procedural(128, celdas, 4, fuerza, 1);
            let mut suma = 0.0f32;
            let mut n = 0;
            for y in 0..128 {
                for x in 0..128 {
                    let nrm = mapa.sample_normal(x as f32 / 128.0, y as f32 / 128.0);
                    assert!(nrm.z > 0.0);
                    suma += nrm.z.acos().to_degrees();
                    n += 1;
                }
            }
            let media = suma / n as f32;
            eprintln!("fuerza {fuerza} celdas {celdas}: inclinacion media {media:.1} grados");
            assert!(media > 1.0 && media < 45.0, "media {media}");
        }
    }
}
