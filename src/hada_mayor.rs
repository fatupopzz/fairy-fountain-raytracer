//! LA GRAN HADA, la que sale de la fuente cuando Link toca la cancion.
//!
//! En el juego, cuando se toca la cancion de Zelda frente a la fuente, el
//! Hada Mayor sale del agua girando, entre risas, y queda flotando
//! en el aire: pelo magenta en dos coletas largas que se levantan
//! como llamas, un traje hecho de enredaderas y hojas, botas de enredadera.
//! Aca pasa lo mismo, y cuando pasa lo decide la cancion: cuando entran las
//! voces (el climax, ver `SyncData::hada_mayor`) sale del agua girando y
//! creciendo, se queda flotando frente a Link, entre el y la Trifuerza, y
//! cuando las voces se van vuelve a girar y se hunde en la fuente.
//!
//! Esta hecha igual que Link (`link.rs`): cubos orientados colgados de
//! huesos, con los brazos y las piernas resueltos por cinematica inversa. La
//! diferencia es que el cuerpo entero se arma en su propio sistema, de pie,
//! y despues se lleva al mundo con una escala y una rotacion: asi el mismo
//! modelo sirve para la figura chiquita que gira saliendo del agua y para
//! la grande que flota.

use crate::caja_orientada::{componer, ejes_de, llevar, CajaOrientada};
use crate::grupo_acotado::GrupoAcotado;
use crate::light::Light;
use crate::link::{codo, Hueso};
use crate::material::Material;
use crate::ray_intersect::RayIntersect;
use crate::sync::SceneParams;
use crate::texture::{campo_fbm, Texture, TextureImage};
use crate::vec3::{cross, normalize, Vec3};
use raylib::prelude::Color;
use std::any::Any;
use std::f32::consts::PI;
use std::sync::Arc;

/// Donde flota la cadera cuando esta afuera del todo: sobre el cuenco del
/// estrado, de donde salio, mirando a Link.
pub const FLOTA: Vec3 = Vec3::new(0.0, 3.05, 0.35);

/// Donde flota cuando bendice a Link: de pie, delante y por encima de el,
/// como en la cinematica, con Link mirandola desde abajo.
const BENDICE: Vec3 = Vec3::new(0.0, 3.3, 1.55);

/// Su tamano a pleno: una vez y media la figura de siete cabezas y media,
/// que es lo que la hace la GRAN hada al lado de Link.
const TAMANIO: f32 = 1.3;

/// La altura de la cadera en su sistema de pie: el pivote del cuerpo.
const CADERA: f32 = 1.25;

#[derive(Clone, Copy)]
enum H {
    Piel,
    Traje,
    Pelo,
    Botas,
    Rojo,
    /// Las puntas de las coletas: mas claras y encendidas, como la punta de
    /// una llama.
    PeloPunta,
}

struct Pieza {
    centro: Vec3,
    ejes: [Vec3; 3],
    tam: Vec3,
    mat: H,
    cara: bool,
}

fn caja(h: &Hueso, centro: Vec3, tam: Vec3, rot: [Vec3; 3], mat: H) -> Pieza {
    Pieza { centro: h.punto(centro), ejes: componer(&h.e, &rot), tam, mat, cara: false }
}

/// Una caja de `a` a `b`, con su eje Y a lo largo del tramo.
fn tramo(a: Vec3, b: Vec3, grueso: f32, mat: H) -> Pieza {
    let largo = (b - a).norm().max(1e-4);
    let y = (b - a) / largo;
    let mut z = cross(&Vec3::new(1.0, 0.0, 0.0), &y);
    if z.norm() < 1e-3 {
        z = cross(&Vec3::new(0.0, 0.0, 1.0), &y);
    }
    let z = normalize(&z);
    let x = cross(&y, &z);
    Pieza {
        centro: (a + b) * 0.5,
        ejes: [x, y, z],
        tam: Vec3::new(grueso, largo + grueso * 0.35, grueso),
        mat,
        cara: false,
    }
}

/// Lo que se mueve del hada, cuadro a cuadro.
#[derive(Clone, Copy)]
struct Pose {
    /// Segundo de la cancion: el ondear del pelo y la respiracion.
    tiempo: f32,
    /// Donde estan las dos manos, en su sistema de pie.
    manos: [Vec3; 2],
    /// La risa: cuanto echa la cabeza hacia atras (0 a 1).
    risa: f32,
    /// Cuanto se abren las coletas, como llamas avivadas (0 a 1).
    llamas: f32,
    /// El pataleo: fase de las piernas, que se mueven alternadas al compas.
    patada: f32,
    /// La risa sacude los hombros: un temblor rapido y chico.
    temblor: f32,
}

/// LAS POSES DE LOS BRAZOS, como posiciones de las dos manos en su sistema
/// de pie. Cambia de pose en cada compas de la cancion, y en
/// la bendicion abre los brazos en cruz sobre Link, como en el juego.
const POSES: [[(f32, f32, f32); 2]; 4] = [
    // Una mano adelante y la otra detras de la cabeza.
    [(-0.44, 1.32, 0.30), (0.30, 2.50, -0.05)],
    // Las dos arriba.
    [(-0.30, 2.55, 0.12), (0.30, 2.55, 0.12)],
    // Una mano tendida hacia Link.
    [(-0.35, 1.70, 0.62), (0.30, 2.50, -0.05)],
    // Los brazos abiertos a los costados.
    [(-0.62, 2.05, 0.30), (0.62, 2.05, 0.30)],
];
/// Los brazos en cruz de la bendicion.
const EN_CRUZ: [(f32, f32, f32); 2] = [(-0.95, 2.02, 0.12), (0.95, 2.02, 0.12)];

fn hash(n: u32, k: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ k.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2545_f491);
    x ^= x >> 13;
    ((x >> 16) & 0xFFFF) as f32 / 65535.0
}

/// La pose del hada en este instante de la cancion.
fn pose_de(p: &SceneParams) -> Pose {
    let t = p.tiempo;
    // Una pose por COMPAS: cuatro tiempos.
    let frase_largo = p.beat_period.max(0.2) * 4.0;
    let frase = (t / frase_largo).floor();
    let dentro = t / frase_largo - frase;
    let elegir = |f: f32| POSES[(hash(f.max(0.0) as u32, 17) * POSES.len() as f32) as usize % POSES.len()];
    let (antes, ahora) = (elegir(frase - 1.0), elegir(frase));
    // Pasa de una pose a la otra en el primer cuarto de la frase.
    let x = suave(dentro * 4.0);
    let v = |a: (f32, f32, f32)| Vec3::new(a.0, a.1, a.2);
    let b = p.bendicion.clamp(0.0, 1.0);
    let mano = |i: usize| {
        let frase = v(antes[i]) + (v(ahora[i]) - v(antes[i])) * x;
        frase + (v(EN_CRUZ[i]) - frase) * b
    };
    // LOS GESTOS: ademas de cambiar de pose en cada compas, las manos nunca
    // se quedan quietas. Cada una dibuja un circulo chico al ritmo del tema,
    // desfasada de la otra, como alguien que habla con las manos.
    let fase = t / p.beat_period.max(0.2) * PI;
    let gesto = |i: usize| {
        let f = fase + i as f32 * 1.7;
        Vec3::new(0.08 * f.cos(), 0.10 * (f * 0.5).sin(), 0.06 * f.sin()) * (1.0 - 0.6 * b)
    };
    Pose {
        tiempo: t,
        manos: [mano(0) + gesto(0), mano(1) + gesto(1)],
        risa: p.pulso * (1.0 - 0.5 * b),
        llamas: p.pulso + 0.4 * b,
        patada: fase * 0.5,
        temblor: p.pulso * (t * 38.0).sin() * (1.0 - b),
    }
}

/// El cuerpo entero, DE PIE y en su propio sistema (pies en el origen,
/// mirando a +Z). Proporciones de figura alta: siete cabezas y media.
fn piezas(pose: &Pose) -> (Vec<Pieza>, Vec<usize>) {
    let mut v = Vec::with_capacity(48);
    // Donde termina cada parte: cada una va en su propio grupo acotado.
    let mut cortes = Vec::new();
    let raiz = Hueso { o: Vec3::zeros(), e: ejes_de(0.0, 0.0, 0.0) };
    let recto = ejes_de(0.0, 0.0, 0.0);
    let t = pose.tiempo;
    let respira = (t * 1.3).sin() * 0.01;

    // ---- EL CUERPO ----
    v.push(caja(&raiz, Vec3::new(0.0, 1.26, 0.0), Vec3::new(0.44, 0.24, 0.28), recto, H::Traje));
    v.push(caja(&raiz, Vec3::new(0.0, 1.47, 0.0), Vec3::new(0.30, 0.20, 0.20), recto, H::Piel));
    v.push(caja(&raiz, Vec3::new(0.0, 1.70 + respira, 0.01), Vec3::new(0.42, 0.30 + respira, 0.26), recto, H::Traje));
    v.push(caja(&raiz, Vec3::new(0.0, 1.93, 0.0), Vec3::new(0.11, 0.16, 0.11), recto, H::Piel));
    // Las hombreras de hojas.
    for lado in [-1.0f32, 1.0] {
        v.push(caja(&raiz, Vec3::new(lado * 0.24, 1.83, 0.0), Vec3::new(0.16, 0.10, 0.22), ejes_de(0.0, 0.0, -lado * 0.3), H::Traje));
    }

    // ---- LA CABEZA ----
    // Inclinada hacia atras y de costado: se esta riendo.
    let cabeza = raiz.hijo(Vec3::new(0.0, 2.0, 0.0), ejes_de(0.15 * (t * 0.7).sin(), -0.20 - 0.30 * pose.risa, 0.12));
    let mut cara = caja(&cabeza, Vec3::new(0.0, 0.16, 0.01), Vec3::new(0.28, 0.33, 0.28), recto, H::Piel);
    cara.cara = true;
    v.push(cara);
    v.push(caja(&cabeza, Vec3::new(0.0, 0.35, -0.01), Vec3::new(0.32, 0.12, 0.32), recto, H::Pelo));
    v.push(caja(&cabeza, Vec3::new(0.0, 0.18, -0.14), Vec3::new(0.32, 0.30, 0.10), recto, H::Pelo));
    for lado in [-1.0f32, 1.0] {
        v.push(caja(&cabeza, Vec3::new(lado * 0.15, 0.20, 0.08), Vec3::new(0.06, 0.24, 0.10), recto, H::Pelo));
    }
    // El mono en la coronilla, de donde nacen las coletas.
    v.push(caja(&cabeza, Vec3::new(0.0, 0.45, -0.05), Vec3::new(0.22, 0.13, 0.22), ejes_de(PI / 4.0, 0.0, 0.0), H::Pelo));

    cortes.push(v.len());
    // LAS COLETAS: dos cadenas de cinco eslabones que salen de la coronilla
    // hacia arriba, afuera y atras, y ondean como llamas. Cada eslabon cuelga
    // del anterior, asi que la onda se suma y la punta es la que mas se mueve.
    for lado in [-1.0f32, 1.0] {
        let mut h = cabeza.hijo(
            Vec3::new(lado * 0.13, 0.38, -0.06),
            ejes_de(0.0, -0.5, -lado * (0.55 + 0.30 * pose.llamas)),
        );
        for k in 0..5 {
            let ancho = 0.21 - k as f32 * 0.03;
            let largo = 0.27;
            let mat = if k >= 3 { H::PeloPunta } else { H::Pelo };
            v.push(caja(&h, Vec3::new(0.0, largo * 0.5, 0.0), Vec3::new(ancho, largo, ancho * 0.8), recto, mat));
            let onda = (t * 2.6 - k as f32 * 0.8 + lado).sin() * 0.22;
            h = h.hijo(Vec3::new(0.0, largo * 0.9, 0.0), ejes_de(onda * 0.5, -0.18 + onda * 0.4, -lado * 0.12));
        }
    }

    cortes.push(v.len());
    // ---- LOS BRAZOS ----
    // Las manos las pone la pose (ver `pose_de`); los codos, la cinematica
    // inversa. Arriba de cada brazo, el pano rojo que le cuelga del hombro.
    for (i, lado) in [-1.0f32, 1.0].into_iter().enumerate() {
        let hombro = Vec3::new(lado * 0.26, 1.82 + 0.02 * pose.temblor, 0.0);
        let mano = pose.manos[i] + Vec3::new(0.0, 0.03 * (t * 1.1 + i as f32).sin(), 0.0);
        // La mano no puede quedar mas lejos de lo que llega el brazo: si la
        // pose la pide mas alla, se la trae hasta el alcance. Si no, el
        // antebrazo se estiraba hasta ella y el brazo se veia partido.
        let hacia_mano = mano - hombro;
        let mano = hombro + hacia_mano * (0.68 / hacia_mano.norm().max(0.68));
        let c = codo(hombro, mano, 0.36, 0.34, Vec3::new(lado, -0.6, -0.3));
        v.push(tramo(hombro, c, 0.10, H::Piel));
        v.push(tramo(hombro - Vec3::new(0.0, 0.02, 0.0), hombro + (c - hombro) * 0.55, 0.16, H::Rojo));
        v.push(tramo(c, mano, 0.09, H::Piel));
        // La pulsera de enredadera y la mano.
        let hacia = (mano - c) / (mano - c).norm().max(1e-4);
        v.push(tramo(mano - hacia * 0.10, mano - hacia * 0.04, 0.115, H::Traje));
        v.push(tramo(mano - hacia * 0.02, mano + hacia * 0.10, 0.07, H::Piel));
    }

    cortes.push(v.len());
    // ---- LAS PIERNAS ----
    // Una casi estirada y la otra doblada hacia atras, cruzada: flota, no
    // esta parada en nada. Muslo con enredadera y botas de enredadera
    // marron, altas hasta la rodilla.
    // Y PATALEA: las rodillas suben y bajan alternadas al compas, y los
    // pies van atras de las rodillas, como quien flota en el agua.
    let (pa, pb) = (pose.patada.sin(), (pose.patada + PI).sin());
    let pies = [
        (Vec3::new(-0.13, 0.64 + 0.10 * pa, 0.10 + 0.14 * pa), Vec3::new(-0.14, 0.08 + 0.06 * pa, 0.0 - 0.10 * pa)),
        (Vec3::new(0.16, 0.70 + 0.10 * pb, 0.22 + 0.12 * pb), Vec3::new(0.12, 0.20 + 0.06 * pb, -0.14 - 0.10 * pb)),
    ];
    for (i, lado) in [-1.0f32, 1.0].into_iter().enumerate() {
        let cadera = Vec3::new(lado * 0.12, 1.16, 0.0);
        let (rodilla, tobillo) = pies[i];
        v.push(tramo(cadera, rodilla, 0.17, H::Traje));
        v.push(tramo(rodilla, tobillo, 0.135, H::Botas));
        // El pie, en punta.
        let adelante = normalize(&(tobillo - rodilla));
        v.push(tramo(tobillo, tobillo + adelante * 0.10 + Vec3::new(0.0, 0.0, 0.14), 0.10, H::Botas));
    }
    cortes.push(v.len());
    (v, cortes)
}

pub struct HadaMayorViva {
    grupo: usize,
    luz: usize,
    /// Las chispas: el estallido al salir, la salpicadura al zambullirse y
    /// la lluvia de la bendicion.
    chispas: usize,
    /// La luz dorada que envuelve a Link mientras recibe la bendicion.
    luz_link: usize,
    /// El rayo de la bendicion: de las manos del hada a Link.
    rayo: usize,
}

/// Cuantas chispas hay.
const CHISPAS: usize = 18;

fn textura_piel() -> TextureImage {
    let ruido = campo_fbm(64, 5, 3, 301);
    TextureImage::pintada(64, 64, move |u, v| {
        let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
        let k = 0.95 + 0.08 * n;
        [1.0 * k, 0.88 * k, 0.86 * k]
    })
}

/// El traje: hojas verdes en capas, como escamas, todas con la punta hacia
/// abajo, sobre los tallos oscuros de la enredadera. Cada hoja tiene su
/// nervadura y se aclara hacia la punta, con un toque dorado: de lejos se
/// lee un verde primavera ordenado, de cerca hojas sueltas.
fn textura_traje() -> TextureImage {
    TextureImage::pintada(128, 128, |u, v| {
        let hash = |x: i32, y: i32, k: u32| {
            let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ k.wrapping_mul(2_246_822_519);
            h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
            ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
        };
        let (gx, gy) = (u * 5.0, v * 5.0);
        // Entre hoja y hoja, el tallo de la enredadera.
        let mut color = [0.16, 0.30, 0.14];
        // Las filas se pintan de arriba hacia abajo, y cada fila tapa a la
        // de arriba: eso las superpone como tejas. Las filas impares van
        // corridas media hoja.
        for dy in [-1i32, 0] {
            let fila = gy.floor() as i32 + dy;
            let corre = if fila.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
            for dx in -1..=1 {
                let col = (gx - corre).floor() as i32 + dx;
                let cx = col as f32 + 0.5 + corre + (hash(col, fila, 1) - 0.5) * 0.15;
                let cy = fila as f32 + 0.55;
                // Cada hoja apenas torcida, para que no quede una grilla.
                let giro = (hash(col, fila, 3) - 0.5) * 0.7;
                let (sg, cg) = giro.sin_cos();
                let (px, py) = (gx - cx, gy - cy);
                let (a, b) = (px * cg + py * sg, -px * sg + py * cg);
                // Una hoja en gota: ancha arriba y en punta abajo.
                let t = (b + 0.55) / 1.2;
                if !(0.0..1.0).contains(&t) {
                    continue;
                }
                let ancho = 0.52 * (t * PI).sin().powf(0.7) * (1.0 - 0.35 * t);
                if a.abs() < ancho {
                    let tono = hash(col, fila, 4);
                    let nervio = if a.abs() < 0.035 { 0.8 } else { 1.0 };
                    let borde = 0.8 + 0.2 * (1.0 - a.abs() / ancho);
                    let k = (0.75 + 0.45 * t) * nervio * borde;
                    color = [
                        (0.30 + 0.12 * tono + 0.10 * t) * k,
                        (0.66 + 0.14 * tono) * k,
                        (0.22 + 0.08 * tono) * k,
                    ];
                }
            }
        }
        color
    })
}

fn textura_pelo(punta: bool) -> TextureImage {
    let ruido = campo_fbm(64, 4, 3, 311);
    TextureImage::pintada(64, 64, move |u, v| {
        let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
        let veta = 0.75 + 0.25 * ((u * 36.0 + n * 7.0).sin() * 0.5 + 0.5);
        if punta {
            // Hacia la punta, del magenta al rosa claro.
            [1.0 * veta, (0.35 + 0.15 * v) * veta, (0.62 + 0.1 * v) * veta]
        } else {
            [0.95 * veta, 0.16 * veta, 0.42 * veta]
        }
    })
}

/// Las botas: enredadera marron oscuro, tallos trenzados.
fn textura_botas() -> TextureImage {
    let ruido = campo_fbm(64, 6, 3, 317);
    TextureImage::pintada(64, 64, move |u, v| {
        let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
        // Tallos en diagonal, cruzados: una trenza.
        let a = ((u + v) * 14.0 + n * 3.0).sin();
        let b = ((u - v) * 14.0 + n * 3.0).sin();
        let tallo = 0.65 + 0.35 * a.max(b);
        [0.34 * tallo, 0.21 * tallo, 0.12 * tallo]
    })
}

/// La cara: ojos grandes con pestanas largas y sombra violeta, cejas finas,
/// labios rojos. Es la cara maquillada del Hada Mayor del juego.
fn textura_cara() -> TextureImage {
    TextureImage::pintada(128, 128, |u, v| {
        let lado = 1.0 - 0.22 * ((u - 0.5) * 2.0).powi(4);
        let mut c = [1.0 * lado, 0.88 * lado, 0.86 * lado];
        // El rubor, rosa, en las mejillas.
        for cx in [0.24f32, 0.76] {
            let d = ((u - cx).powi(2) + (v - 0.66).powi(2)).sqrt();
            let k = (1.0 - d / 0.11).clamp(0.0, 1.0) * 0.30;
            c = [c[0], c[1] * (1.0 - k), c[2] * (1.0 - k * 0.6)];
        }
        for (cx, s) in [(0.31f32, -1.0f32), (0.69, 1.0)] {
            // La sombra de ojos, violeta, arriba del ojo.
            let (sx, sy) = ((u - cx) / 0.15, (v - 0.43) / 0.07);
            if sx * sx + sy * sy < 1.0 {
                c = [0.75, 0.45, 0.85];
            }
            let (dx, dy) = ((u - cx) / 0.12, (v - 0.50) / 0.075);
            let dy = dy + dx * s * 0.08;
            let r = dx * dx + dy * dy;
            if r < 1.0 {
                c = [0.98, 0.98, 0.95];
                let (ix, iy) = ((u - cx) / 0.055, (v - 0.505) / 0.065);
                if ix * ix + iy * iy < 1.0 {
                    // El iris violeta, mas claro abajo, como un ojo que brilla.
                    let k = 0.7 + 0.5 * iy.max(0.0);
                    c = [0.45 * k, 0.22 * k, 0.70 * k];
                    if ix * ix + iy * iy < 0.3 {
                        c = [0.05, 0.02, 0.06];
                    }
                    if ((u - cx + 0.015) / 0.013).powi(2) + ((v - 0.49) / 0.013).powi(2) < 1.0 {
                        c = [1.0, 1.0, 1.0];
                    }
                }
            } else if r < 1.6 && dy < 0.2 {
                c = [0.10, 0.04, 0.08];
            }
            // Las pestanas: tres rayitas hacia afuera y arriba.
            for k in 0..3 {
                let ang = 0.5 + k as f32 * 0.35;
                let (bx, by) = (cx + s * 0.11, 0.47 - k as f32 * 0.012);
                let (px, py) = (u - bx, v - by);
                let a = px * s * ang.cos() - py * ang.sin();
                let b = px * s * ang.sin() + py * ang.cos();
                if (0.0..0.05).contains(&a) && b.abs() < 0.007 {
                    c = [0.10, 0.04, 0.08];
                }
            }
            // La ceja, fina, alta y arqueada: dulce, no enojada.
            let x = (u - cx) / 0.13;
            let ey = (v - (0.335 - 0.035 * (1.0 - x * x) + s * x * 0.012)) / 0.011;
            if ((u - cx) / 0.13).abs() < 1.0 && ey.abs() < 1.0 {
                c = [0.70, 0.12, 0.35];
            }
        }
        // Los labios, rosa fuerte, en una sonrisa: la comisura sube.
        let x = (u - 0.5) / 0.10;
        let medio = 0.80 - 0.035 * x * x;
        let grueso = 0.022 * (1.0 - x * x).max(0.0).sqrt();
        if x.abs() < 1.0 && (v - medio).abs() < grueso + 0.004 {
            c = if (v - medio).abs() < 0.004 { [0.45, 0.06, 0.18] } else { [0.92, 0.25, 0.45] };
        }
        c
    })
}

/// Arma a la Gran Hada (escondida: aparece con la cancion) y su luz.
pub fn armar(objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>, luces: &mut Vec<Light>) -> HadaMayorViva {
    let img = |t: TextureImage| Texture::ImageTexture(Arc::new(t), Color::WHITE, (0.0, 0.0));
    let mats = [
        // La piel brilla apenas, rosada: es un ser de luz, no de carne.
        Material::new([1.0, 0.25, 0.0, 0.0], 30.0, 0.0, img(textura_piel()), Some(Color::new(16, 8, 12, 255))),
        Material::new([1.0, 0.30, 0.0, 0.0], 30.0, 0.0, img(textura_traje()), Some(Color::new(4, 12, 4, 255))),
        // El pelo brilla apenas: es lo que la hace leerse magica y no un
        // maniqui, y lo que mas halo le saca al bloom.
        Material::new([1.0, 0.5, 0.0, 0.0], 40.0, 0.0, img(textura_pelo(false)), Some(Color::new(55, 6, 28, 255))),
        Material::new([1.0, 0.2, 0.0, 0.0], 20.0, 0.0, img(textura_botas()), None),
        // El pano rojo de los brazos.
        Material::new(
            [1.0, 0.35, 0.0, 0.0],
            25.0,
            0.0,
            Texture::Solid(Color::new(220, 50, 110, 255)),
            None,
        ),
        Material::new([1.0, 0.5, 0.0, 0.0], 40.0, 0.0, img(textura_pelo(true)), Some(Color::new(80, 20, 50, 255))),
    ];
    let cara = Material::new([1.0, 0.25, 0.0, 0.0], 30.0, 0.0, img(textura_cara()), Some(Color::new(16, 8, 12, 255)));

    let reposo = Pose {
        tiempo: 0.0,
        manos: [Vec3::new(-0.44, 1.32, 0.3), Vec3::new(0.3, 2.5, -0.05)],
        risa: 0.0,
        llamas: 0.0,
        patada: 0.0,
        temblor: 0.0,
    };
    let (todas, cortes) = piezas(&reposo);
    let mut cajas = todas.iter().map(|p| {
        let mut c = CajaOrientada::nueva(FLOTA, Vec3::zeros(), p.ejes, mats[p.mat as usize].clone());
        if p.cara {
            c = c.con_frente(cara.clone());
        }
        Box::new(c) as Box<dyn RayIntersect + Send + Sync>
    });
    let mut desde = 0;
    let partes: Vec<Box<dyn RayIntersect + Send + Sync>> = cortes
        .iter()
        .map(|&hasta| {
            let parte: Vec<_> = cajas.by_ref().take(hasta - desde).collect();
            desde = hasta;
            Box::new(GrupoAcotado::new(parte)) as Box<dyn RayIntersect + Send + Sync>
        })
        .collect();
    let grupo = objetos.len();
    objetos.push(Box::new(GrupoAcotado::new(partes)));

    let luz = luces.len();
    luces.push(Light::new(FLOTA + Vec3::new(0.0, 0.4, 1.4), Color::new(255, 160, 220, 255), 0.0).con_alcance(1.6));

    // Las chispas, escondidas (tamano cero) hasta que hacen falta.
    let chispa = Material::new(
        [0.0, 0.0, 0.0, 0.0],
        1.0,
        0.0,
        Texture::Solid(Color::WHITE),
        Some(Color::BLACK),
    );
    let chispas_cajas: Vec<Box<dyn RayIntersect + Send + Sync>> = (0..CHISPAS)
        .map(|_| {
            Box::new(CajaOrientada::nueva(FLOTA, Vec3::zeros(), ejes_de(0.0, 0.0, 0.0), chispa.clone()))
                as Box<dyn RayIntersect + Send + Sync>
        })
        .collect();
    let chispas = objetos.len();
    objetos.push(Box::new(GrupoAcotado::new(chispas_cajas)));

    let luz_link = luces.len();
    luces.push(
        Light::new(crate::link::PIES + Vec3::new(0.0, 1.9, 0.6), Color::new(255, 215, 120, 255), 0.0)
            .con_alcance(1.2),
    );
    // EL RAYO DE LA BENDICION: un haz dorado translucido (suma su luz, no
    // tapa) que baja de las manos del hada a la cabeza de Link.
    let velo = Material::new([0.0, 0.0, 0.0, 1.0], 1.0, 1.0, Texture::Solid(Color::WHITE), Some(Color::BLACK));
    let mut haz = crate::cylinder::CilindroOrientado::nuevo(FLOTA, FLOTA + Vec3::new(0.0, 0.1, 0.0), 0.0, 1.0, velo);
    haz.set_visible(false);
    let rayo = objetos.len();
    objetos.push(Box::new(GrupoAcotado::new(vec![Box::new(haz) as Box<dyn RayIntersect + Send + Sync>])));
    HadaMayorViva { grupo, luz, chispas, luz_link, rayo }
}

fn suave(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Donde esta y como esta girada el hada en este instante: la cadera, los
/// ejes, la escala y la presencia. Lo usan tambien los anillos, que en el
/// climax pasan a girar alrededor de ella.
pub fn colocacion(p: &SceneParams) -> (Vec3, [Vec3; 3], f32, f32) {
    let presencia = p.hada.clamp(0.0, 1.0);
    let t = p.tiempo;
    let b = suave(p.bendicion);

    // SALE DEL AGUA GIRANDO: con poca presencia esta abajo, chica y dando
    // vueltas; con toda, arriba, grande y quieta. Sale y entra por el
    // mismo camino, asi que irse es lo mismo al reves.
    let sube = suave(presencia * 1.25);
    let escala = (0.12 + 0.88 * suave(presencia)) * TAMANIO * (1.0 + 0.025 * p.pulso);
    let giro = (1.0 - suave(presencia)) * 3.0 * PI;
    // LA VOLTERETA: mientras sale del agua da una vuelta entera hacia atras,
    // ademas de girar; se completa justo cuando llega arriba.
    let voltereta = (1.0 - suave((presencia - 0.15) / 0.55)) * 2.0 * PI * (presencia > 0.02) as i32 as f32;
    // Flota, y ademas BAILA: sube en cada golpe y se mece de costado al
    // compas (un vaiven cada dos tiempos), mas cuando la cancion empuja.
    let beat = p.beat_period.max(0.2);
    let vaiven = (t / beat * PI).sin() * (0.35 + 0.65 * p.energia_suave.clamp(0.0, 1.0));
    let flota = Vec3::new(
        0.12 * vaiven,
        0.16 * (t * 0.9).sin() + 0.06 * (t * 2.3).sin() + 0.10 * p.pulso,
        0.05 * (t * 0.7).cos(),
    ) * sube;
    let base = Vec3::new(0.0, crate::fuente::CUENCO_Y - 1.2, 0.0);
    // Arriba del cuenco, y para bendecir avanza sobre Link.
    let arriba = FLOTA + (BENDICE - FLOTA) * b;
    let donde = base + (arriba - base) * sube + flota;
    // DE PIE, FLOTANDO. Estuvo recostada de costado, como en una toma de la
    // cinematica, pero quieta en esa pose se leia rara: ahora flota derecha,
    // con las piernas colgando, y lo unico que la inclina es el baile.
    let r = ejes_de(
        giro + (0.10 * (t * 0.5).sin() + 0.18 * vaiven) * (1.0 - b),
        -0.12 * b + 0.05 * (t * 0.6).sin() - voltereta,
        0.07 * (t * 0.8).sin() + 0.08 * vaiven,
    );
    (donde, r, escala, presencia)
}

impl HadaMayorViva {
    pub fn actualizar(&self, objetos: &mut [Box<dyn RayIntersect + Send + Sync>], luces: &mut [Light], p: &SceneParams) {
        let t = p.tiempo;
        let (donde, r, escala, presencia) = colocacion(p);
        let pose = pose_de(p);
        let (nuevas, _) = piezas(&pose);
        let al_mundo = |local: Vec3| donde + llevar(&r, &((local - Vec3::new(0.0, CADERA, 0.0)) * escala));

        if let Some(g) = objetos
            .get_mut(self.grupo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        {
            let mut piezas_nuevas = nuevas.iter();
            for parte in g.children_mut() {
                let Some(parte) = (parte.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>() else {
                    continue;
                };
                for (hijo, pieza) in parte.children_mut().iter_mut().zip(piezas_nuevas.by_ref()) {
                if let Some(c) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<CajaOrientada>() {
                    c.colocar(al_mundo(pieza.centro), componer(&r, &pieza.ejes));
                    // Sin presencia, tamano cero: no existe para ningun rayo.
                    c.medio = if presencia > 0.005 { pieza.tam * (0.5 * escala) } else { Vec3::zeros() };
                    if matches!(pieza.mat, H::Pelo) {
                        let k = 0.6 + 0.4 * p.pulso + 0.3 * p.bendicion;
                        c.material.emission_color =
                            Some(Color::new((70.0 * k) as u8, (8.0 * k) as u8, (36.0 * k) as u8, 255));
                    }
                }
                }
                parte.recalcular_caja(0.0);
            }
            g.recalcular_caja(0.0);
        }

        // Su luz: rosa, delante de ella, mirandola. Es la que la saca del
        // contraluz de la fuente.
        if let Some(luz) = luces.get_mut(self.luz) {
            luz.position = donde + Vec3::new(0.0, 0.5, 1.3);
            luz.intensity = 1.6 * suave(presencia) * (0.85 + 0.3 * p.pulso);
        }
        // La luz dorada sobre Link mientras recibe el poder.
        if let Some(luz) = luces.get_mut(self.luz_link) {
            luz.intensity = 2.2 * p.bendicion * (0.8 + 0.4 * p.pulso);
        }

        // ---- EL RAYO ----
        let entre_manos = (al_mundo(pose.manos[0]) + al_mundo(pose.manos[1])) * 0.5;
        let cabeza_link = crate::link::PIES + Vec3::new(0.0, 1.75, 0.0);
        if let Some(g) = objetos
            .get_mut(self.rayo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        {
            let b = p.bendicion;
            for hijo in g.children_mut() {
                if let Some(c) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<crate::cylinder::CilindroOrientado>() {
                    c.set_visible(b > 0.05);
                    if b > 0.05 {
                        c.recolocar(cabeza_link, entre_manos);
                        c.set_radio(0.16 * b * (0.85 + 0.3 * p.pulso));
                        let k = b * (0.55 + 0.45 * p.pulso);
                        c.material_mut().emission_color =
                            Some(Color::new((255.0 * k) as u8, (205.0 * k) as u8, (110.0 * k) as u8, 255));
                    }
                }
            }
            g.recalcular_caja(0.0);
        }

        // ---- LAS CHISPAS ----
        // Tres usos, en orden de prioridad: el ESTALLIDO cuando sale del
        // agua (salen del cuenco en todas direcciones), la SALPICADURA cuando
        // se zambulle, y la LLUVIA DE LA BENDICION, que cae de sus manos
        // abiertas sobre Link. Todo es funcion de la edad del evento.
        let cuenco = Vec3::new(0.0, crate::fuente::CUENCO_Y, 0.0);
        let link = crate::link::PIES + Vec3::new(0.0, 1.45, 0.0);
        let manos = [al_mundo(pose.manos[0]), al_mundo(pose.manos[1])];
        let Some(g) = objetos
            .get_mut(self.chispas)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        else {
            return;
        };
        for (k, hijo) in g.children_mut().iter_mut().enumerate() {
            let Some(c) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<CajaOrientada>() else {
                continue;
            };
            let h = |i: u32| hash(k as u32, i);
            // Una direccion al azar, casi siempre hacia arriba y afuera.
            let ang = h(1) * 2.0 * PI;
            let dir = normalize(&Vec3::new(ang.cos(), 0.6 + h(2) * 1.2, ang.sin()));
            let (pos, tam, color) = if (0.0..1.8).contains(&p.hada_salio) {
                let e = p.hada_salio;
                let vuelo = (2.8 + h(3) * 1.5) * e;
                let vida = 1.0 - e / 1.8;
                (cuenco + dir * vuelo - Vec3::new(0.0, 1.2 * e * e, 0.0), 0.10 * vida, (255.0, 150.0, 230.0))
            } else if (0.0..1.3).contains(&p.hada_entro) {
                let e = p.hada_entro;
                let vuelo = (1.8 + h(3)) * e;
                let vida = 1.0 - e / 1.3;
                (cuenco + dir * vuelo - Vec3::new(0.0, 2.0 * e * e, 0.0), 0.08 * vida, (150.0, 255.0, 200.0))
            } else if p.bendicion > 0.02 {
                // Cada chispa recorre su propio arco de una mano a Link,
                // desfasada de las demas, asi la lluvia es continua.
                let s = (t * 0.6 + k as f32 / CHISPAS as f32).fract();
                let desde = manos[k % 2];
                let medio = (desde + link) * 0.5 + Vec3::new((h(4) - 0.5) * 0.8, 0.6, (h(5) - 0.5) * 0.4);
                let a = desde + (medio - desde) * s;
                let b = medio + (link - medio) * s;
                let brillo = (s * PI).sin();
                (a + (b - a) * s, 0.075 * brillo * p.bendicion, (255.0, 220.0, 130.0))
            } else {
                (cuenco, 0.0, (0.0, 0.0, 0.0))
            };
            let giro = ejes_de(t * 3.0 + k as f32, t * 2.0, 0.7);
            c.colocar(pos, giro);
            c.medio = Vec3::new(tam, tam, tam) * 0.5;
            let k = 1.0;
            c.material.emission_color = Some(Color::new((color.0 * k) as u8, (color.1 * k) as u8, (color.2 * k) as u8, 255));
        }
        g.recalcular_caja(0.0);
    }
}
