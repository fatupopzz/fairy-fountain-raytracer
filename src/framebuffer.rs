use crate::camera::Camera;
use nalgebra_glm::{dot, normalize};
use rayon::prelude::*;
use raylib::prelude::*;

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    buffer: Vec<Color>,
    /// Distancia del impacto mas cercano de cada pixel, en el mismo orden
    /// que `buffer`. f32::MAX quiere decir "el rayo no le pego a nada".
    /// El post-procesado lo usa para la niebla.
    depth: Vec<f32>,
    /// El resultado ACUMULADO de los cuadros anteriores, en flotante (ver
    /// `acumular`). En f32 y no en `Color` porque se le suman fracciones
    /// de cuadro: en 8 bits, mezclar al 25% pierde todo lo que valga menos
    /// de cuatro niveles y la acumulacion se estanca.
    historia: Vec<[f32; 3]>,
    /// Si `historia` tiene algo util. En falso, el primer `acumular` se
    /// limita a copiar el cuadro.
    hay_historia: bool,
    /// El volcado a bytes para la GPU, reusado cuadro a cuadro. Antes se
    /// armaba un `Vec` nuevo en cada vuelta: medio megabyte pedido y
    /// devuelto al sistema sesenta veces por minuto, para escribir siempre
    /// lo mismo. Vive aca y se sobrescribe.
    bytes: Vec<u8>,
    /// Donde se va escribiendo la historia nueva mientras se lee la vieja.
    /// Hace falta porque la reproyeccion lee la historia en un punto
    /// CUALQUIERA, no en el mismo pixel: escribiendo encima, un pixel ya
    /// resuelto contaminaria al siguiente. Vive como campo y no como
    /// variable local para no pedirle al sistema un megabyte y medio en
    /// cada cuadro.
    historia_nueva: Vec<[f32; 3]>,
    background: Color,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize, background: Color) -> Self {
        Framebuffer {
            width,
            height,
            buffer: vec![background; width * height],
            depth: vec![f32::MAX; width * height],
            bytes: vec![0; width * height * 4],
            historia: vec![[0.0; 3]; width * height],
            historia_nueva: vec![[0.0; 3]; width * height],
            hay_historia: false,
            background,
        }
    }

    pub fn clear(&mut self) {
        for px in self.buffer.iter_mut() {
            *px = self.background;
        }
        for t in self.depth.iter_mut() {
            *t = f32::MAX;
        }
    }

    /// Las filas `[desde, hasta)` del color y de la profundidad, para
    /// escribirlas EN PARALELO desde el trazador.
    ///
    /// Antes el trazado juntaba la banda entera en un `Vec` y despues la
    /// copiaba pixel por pixel, con una division y un modulo en cada uno
    /// para sacar la fila y la columna. Eso son dos reservas de memoria y
    /// ciento veinte mil copias por cuadro, todas en un solo hilo, para
    /// mover datos que ya estaban calculados. Prestando las filas, cada
    /// hilo escribe directamente donde va.
    ///
    /// Los dos tramos salen juntos porque son dos prestamos mutables del
    /// mismo `self`: pedirlos de a uno no compila.
    pub fn filas_mut(&mut self, desde: usize, hasta: usize) -> (&mut [Color], &mut [f32]) {
        let a = desde * self.width;
        let b = hasta * self.width;
        (&mut self.buffer[a..b], &mut self.depth[a..b])
    }

    /// ANTIALIASING TEMPORAL: mezcla este cuadro con los anteriores.
    ///
    /// La idea es repartir el muestreo en el TIEMPO en vez de en el cuadro.
    /// Cada cuadro se traza con el rayo corrido un poquito dentro del pixel
    /// (el jitter que arma `render_rows`), asi que dos cuadros seguidos son
    /// dos muestras distintas del mismo pixel; promediandolos sale lo mismo
    /// que el antialiasing de 2x2 pero SIN trazar cuatro rayos, porque las
    /// otras muestras ya se trazaron en los cuadros anteriores.
    ///
    /// LA REPROYECCION es lo que lo hace funcionar con la camara en
    /// movimiento, que aca es SIEMPRE: el pendulo no se detiene nunca. El
    /// pixel (x, y) de este cuadro no mira el mismo punto del mundo que el
    /// (x, y) del anterior, asi que mezclarlos directo emborrona. Pero SI
    /// se sabe que punto del mundo mira: es el origen de la camara mas su
    /// rayo por la distancia que el trazador dejo en el depth buffer. Ese
    /// punto se proyecta con la camara del cuadro PASADO y ahi es donde
    /// vive su historia. Medido en esta escena con `--taa`, el pendulo
    /// mueve la imagen 2.6 pixeles por cuadro a 20 cuadros por segundo:
    /// sin reproyectar, la historia no serviria casi nunca.
    ///
    /// EL PROBLEMA DEL FANTASMA y como se resuelve. Lo que se movio o
    /// cambio de color (un hada que cae, algo que estaba tapado y
    /// aparecio) deja un rastro: la historia recuerda lo que habia antes.
    /// El remedio es el estandar: antes de mezclar, la historia se RECORTA
    /// a la caja de colores de los ocho vecinos del pixel en el cuadro
    /// nuevo. Si lo que recuerda no se parece a nada de lo que hay ahora
    /// alrededor, esa parte de la escena cambio y el recorte lo tira.
    /// Cuesta ocho lecturas por pixel y es lo que separa un antialiasing
    /// temporal de un cuadro borroneado.
    ///
    /// `peso_nuevo` es cuanto pesa el cuadro recien trazado: 1.0 lo deja
    /// tal cual (sin acumular) y 0.25 lo mezcla con tres cuartos de
    /// historia. `antes` en `None` (el primer cuadro, o despues de un
    /// salto) descarta la historia.
    pub fn acumular(
        &mut self,
        peso_nuevo: f32,
        ahora: &Camera,
        antes: Option<&Camera>,
        jitter: (f32, f32),
    ) {
        let peso = peso_nuevo.clamp(0.0, 1.0);

        // Sin historia util, o pidiendo el cuadro crudo: se copia y listo.
        let (Some(antes), true) = (antes, self.hay_historia && peso < 1.0) else {
            for (h, px) in self.historia.iter_mut().zip(self.buffer.iter()) {
                *h = [px.r as f32, px.g as f32, px.b as f32];
            }
            self.hay_historia = true;
            return;
        };

        let (w, h) = (self.width, self.height);

        // Los campos por separado: el bucle escribe en `historia_nueva`
        // mientras lee `historia`, `buffer` y `depth`, y pidiendo `self`
        // entero el compilador ve un prestamo mutable de todo.
        let Framebuffer {
            buffer,
            depth,
            historia,
            historia_nueva,
            ..
        } = self;

        let (der_hoy, arr_hoy, ade_hoy) = ahora.basis();
        let (der_ayer, arr_ayer, ade_ayer) = antes.basis();

        // El mismo lente que arma los rayos en `render_rows`: 45 grados.
        let escala = (std::f32::consts::PI / 8.0).tan();
        let aspecto = w as f32 / h as f32;

        // Cada fila es independiente: solo lee de los buffers viejos y
        // escribe en su propio tramo del nuevo. Va en paralelo por lo
        // mismo que el trazado, y por la misma razon: son 120 mil pixeles
        // con nueve lecturas cada uno, y en serie costaba casi cuatro
        // milisegundos, el diez por ciento del cuadro.
        historia_nueva
            .par_chunks_mut(w)
            .enumerate()
            .for_each(|(y, salida)| {
            for x in 0..w {
                let i = y * w + x;

                // La caja de colores de los vecinos en el cuadro NUEVO.
                let mut minimo = [255.0f32; 3];
                let mut maximo = [0.0f32; 3];
                for vy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                    for vx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                        let v = buffer[vy * w + vx];
                        for (c, canal) in [v.r, v.g, v.b].into_iter().enumerate() {
                            let canal = canal as f32;
                            minimo[c] = minimo[c].min(canal);
                            maximo[c] = maximo[c].max(canal);
                        }
                    }
                }

                let actual = buffer[i];
                let nuevo = [actual.r as f32, actual.g as f32, actual.b as f32];

                // --- Donde estaba este punto en el cuadro anterior ---
                //
                // 1. Se rehace el rayo que trazo este pixel, con el mismo
                //    jitter, y se avanza por el la distancia del impacto.
                //    Lo que no le pego a nada (el cielo) se manda muy
                //    lejos: ahi la reproyeccion queda gobernada por el
                //    giro de la camara, que es lo correcto para un fondo
                //    infinito.
                let sx = ((2.0 * (x as f32 + jitter.0)) / w as f32 - 1.0) * aspecto * escala;
                let sy = (1.0 - (2.0 * (y as f32 + jitter.1)) / h as f32) * escala;
                let rayo = normalize(&(der_hoy * sx + arr_hoy * sy + ade_hoy));
                let distancia = depth[i];
                let distancia = if distancia.is_finite() { distancia } else { 1000.0 };
                let punto = ahora.position + rayo * distancia;

                // 2. Y se lo proyecta con la camara de ayer, invirtiendo
                //    las mismas cuentas.
                let d = punto - antes.position;
                let profundidad = dot(&d, &ade_ayer);
                let peso_pixel = if profundidad <= 1e-3 {
                    // Quedaba DETRAS de la camara de ayer: no hay historia.
                    1.0
                } else {
                    let ndc_x = dot(&d, &der_ayer) / profundidad / (aspecto * escala);
                    let ndc_y = dot(&d, &arr_ayer) / profundidad / escala;
                    let px = (ndc_x + 1.0) / 2.0 * w as f32 - jitter.0;
                    let py = (1.0 - ndc_y) / 2.0 * h as f32 - jitter.1;

                    if px < 0.0 || py < 0.0 || px > (w - 1) as f32 || py > (h - 1) as f32 {
                        // Entro por un borde: tampoco hay historia.
                        1.0
                    } else {
                        // Bilineal sobre la historia, recortada a la caja
                        // de los vecinos.
                        let (x0, y0) = (px.floor() as usize, py.floor() as usize);
                        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                        let (tx, ty) = (px - x0 as f32, py - y0 as f32);

                        let mut viejo = [0.0f32; 3];
                        for c in 0..3 {
                            let arriba = historia[y0 * w + x0][c]
                                + (historia[y0 * w + x1][c] - historia[y0 * w + x0][c]) * tx;
                            let abajo = historia[y1 * w + x0][c]
                                + (historia[y1 * w + x1][c] - historia[y1 * w + x0][c]) * tx;
                            viejo[c] = (arriba + (abajo - arriba) * ty).clamp(minimo[c], maximo[c]);
                        }

                        for c in 0..3 {
                            salida[x][c] = viejo[c] + (nuevo[c] - viejo[c]) * peso;
                        }
                        0.0
                    }
                };

                if peso_pixel > 0.0 {
                    salida[x] = nuevo;
                }
            }
        });

        std::mem::swap(&mut self.historia, &mut self.historia_nueva);

        for (px, h) in self.buffer.iter_mut().zip(self.historia.iter()) {
            *px = Color::new(
                h[0].clamp(0.0, 255.0) as u8,
                h[1].clamp(0.0, 255.0) as u8,
                h[2].clamp(0.0, 255.0) as u8,
                255,
            );
        }
    }

    /// La profundidad a la que el canal alpha llega a 255.
    ///
    /// La escena mide unas 36 unidades de lado a lado y la camara orbita a
    /// 10 del centro, asi que el impacto mas lejano que se puede ver anda
    /// por 30. Cerrando en 50 el rango util queda dentro de los 256 niveles
    /// sin que nada real se sature contra el techo.
    pub const PROFUNDIDAD_MAXIMA: f32 = 50.0;

    /// Los pixeles como bytes RGBA crudos, listos para subir a la textura
    /// que consume la GPU: `[R, G, B, A, R, G, B, A, ...]`, de arriba a
    /// abajo y de izquierda a derecha.
    ///
    /// EL ALPHA NO ES OPACIDAD, ES LA PROFUNDIDAD del impacto (0 = pegado a
    /// la camara, 255 = el fondo). Es el unico canal libre que queda, y el
    /// post-procesado lo necesita: la niebla se calcula en la GPU y sin la
    /// distancia no puede saber que velar. Mandarlo por una segunda textura
    /// costaria otra subida de 800x600 por cuadro para ganar precision que
    /// la niebla no usa.
    ///
    /// Es ademas la via rapida para presentar el cuadro. `to_image` arma una
    /// Image de raylib pintando pixel por pixel, o sea 480 mil llamadas a la
    /// libreria de C por cuadro, y eso salia ~400 ms de hilo principal
    /// congelado en cada render. Aca se llena un Vec y se manda entero.
    pub fn to_rgba_bytes(&mut self) -> &[u8] {
        let destino = self.bytes.chunks_exact_mut(4);

        for ((px, t), salida) in self.buffer.iter().zip(self.depth.iter()).zip(destino) {
            // El rayo que no le pego a nada trae f32::MAX; el clamp lo deja
            // en 255, que es lo que corresponde: infinitamente lejos.
            let alpha = ((t / Self::PROFUNDIDAD_MAXIMA).clamp(0.0, 1.0) * 255.0) as u8;

            salida[0] = px.r;
            salida[1] = px.g;
            salida[2] = px.b;
            salida[3] = alpha;
        }

        &self.bytes
    }

    /// Igual que `to_rgba_bytes` pero con el alpha en 255, para exportar
    /// PNGs. Un visor de imagenes lee el alpha como transparencia, asi que
    /// guardar la profundidad ahi daria una imagen agujereada.
    pub fn to_rgba_opaco(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.buffer.len() * 4);

        for px in &self.buffer {
            bytes.push(px.r);
            bytes.push(px.g);
            bytes.push(px.b);
            bytes.push(255);
        }

        bytes
    }

    /// Convierte el buffer en una Image de raylib. Solo se usa para CREAR
    /// las texturas al arranque; para actualizarlas esta `to_rgba_bytes`.
    pub fn to_image(&self) -> Image {
        let mut image =
            Image::gen_image_color(self.width as i32, self.height as i32, self.background);

        for y in 0..self.height {
            for x in 0..self.width {
                image.draw_pixel(x as i32, y as i32, self.buffer[y * self.width + x]);
            }
        }

        image
    }
}
