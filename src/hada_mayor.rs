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
    /// La cabeza: guinada, cabeceo (negativo, hacia atras: la risa) y
    /// alabeo.
    cabeza: (f32, f32, f32),
    /// Cuanto se abren las coletas, como llamas avivadas (0 a 1).
    llamas: f32,
    /// Las piernas: la fase del pataleo y cuanto patalea.
    patada: f32,
    pataleo: f32,
    /// La risa sacude los hombros: un temblor rapido y chico.
    temblor: f32,
}

/// UN MOVIMIENTO DE SU BAILE: donde van las manos (en su sistema de pie),
/// como lleva la cabeza, cuanto se rie, cuanto se abren las coletas, cuanto
/// patalea, y como lleva el cuerpo entero (guinada y alabeo).
#[derive(Clone, Copy)]
struct Mov {
    manos: [(f32, f32, f32); 2],
    cabeza: (f32, f32, f32),
    risa: f32,
    llamas: f32,
    pataleo: f32,
    cuerpo: (f32, f32),
    /// Cuanto aletean los brazos, como alas lentas (0 a 1).
    aleteo: f32,
}

const ARRIBA: Mov = Mov {
    manos: [(-0.30, 2.55, 0.12), (0.30, 2.55, 0.12)],
    cabeza: (0.0, -0.25, 0.0),
    risa: 0.3,
    llamas: 1.0,
    pataleo: 0.6,
    cuerpo: (0.0, 0.0),
    aleteo: 0.0,
};
/// LA RISA del juego: una mano en la boca, la otra en la cadera, la cabeza
/// echada atras y los hombros sacudiendose.
const RIE: Mov = Mov {
    manos: [(-0.10, 2.10, 0.26), (0.34, 1.30, 0.06)],
    cabeza: (0.25, -0.45, 0.15),
    risa: 1.0,
    llamas: 0.7,
    pataleo: 1.0,
    cuerpo: (-0.35, 0.10),
    aleteo: 0.0,
};
/// Le tiende una mano a Link, con la otra detras de la cabeza.
const SALUDA: Mov = Mov {
    manos: [(-0.35, 1.72, 0.62), (0.30, 2.50, -0.05)],
    cabeza: (-0.25, 0.10, -0.12),
    risa: 0.2,
    llamas: 0.5,
    pataleo: 0.5,
    cuerpo: (0.30, -0.08),
    aleteo: 0.0,
};
/// Junta las manos delante del pecho: esta juntando el poder.
const JUNTA: Mov = Mov {
    manos: [(-0.05, 1.78, 0.34), (0.05, 1.78, 0.34)],
    cabeza: (0.0, 0.22, 0.0),
    risa: 0.0,
    llamas: 0.3,
    pataleo: 0.2,
    cuerpo: (0.0, 0.0),
    aleteo: 0.0,
};
/// Los brazos en cruz de la bendicion.
const CRUZ: Mov = Mov {
    manos: [(-0.95, 2.02, 0.12), (0.95, 2.02, 0.12)],
    cabeza: (0.0, 0.18, 0.0),
    risa: 0.0,
    llamas: 1.0,
    pataleo: 0.3,
    cuerpo: (0.0, 0.0),
    aleteo: 0.0,
};
/// La cruz, pero los brazos suben y bajan despacio, como alas.
const ALAS: Mov = Mov {
    manos: [(-0.85, 1.90, 0.02), (0.85, 1.90, 0.02)],
    cabeza: (0.0, -0.10, 0.0),
    risa: 0.3,
    llamas: 1.0,
    pataleo: 0.4,
    cuerpo: (0.0, 0.0),
    aleteo: 1.0,
};
/// Se inclina hacia Link con las dos manos tendidas hacia el.
const OFRECE: Mov = Mov {
    manos: [(-0.30, 1.62, 0.62), (0.30, 1.62, 0.62)],
    cabeza: (0.0, 0.32, 0.0),
    risa: 0.0,
    llamas: 0.6,
    pataleo: 0.3,
    cuerpo: (0.0, 0.0),
    aleteo: 0.0,
};
/// Una mano al cielo y la otra hacia Link: el poder pasa por ella.
const PASA: Mov = Mov {
    manos: [(-0.25, 2.60, 0.05), (0.45, 1.65, 0.55)],
    cabeza: (-0.15, -0.30, 0.10),
    risa: 0.5,
    llamas: 1.0,
    pataleo: 0.7,
    cuerpo: (-0.20, 0.06),
    aleteo: 0.0,
};

/// LA COREOGRAFIA, en segundos desde que empieza a salir del agua. No va al
/// compas: cada movimiento dura lo que dura una frase de la cancion, y pasa
/// al siguiente en un segundo, como alguien que baila y no un metronomo.
/// Despues del ultimo vuelve a repetir los de la bendicion.
const COREOGRAFIA: [(f32, Mov); 10] = [
    (0.0, ARRIBA),
    (3.2, RIE),
    (6.0, SALUDA),
    (8.3, JUNTA),
    (10.0, CRUZ),
    (13.6, ALAS),
    (17.4, OFRECE),
    (20.2, PASA),
    (23.0, CRUZ),
    (26.0, ARRIBA),
];
/// Desde cual se repite si dura mas.
const REPITE_DESDE: usize = 4;

fn hash(n: u32, k: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9e37_79b9) ^ k.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2545_f491);
    x ^= x >> 13;
    ((x >> 16) & 0xFFFF) as f32 / 65535.0
}

fn mezclar(a: &Mov, b: &Mov, x: f32) -> Mov {
    let l = |a: f32, b: f32| a + (b - a) * x;
    let l3 = |a: (f32, f32, f32), b: (f32, f32, f32)| (l(a.0, b.0), l(a.1, b.1), l(a.2, b.2));
    Mov {
        manos: [l3(a.manos[0], b.manos[0]), l3(a.manos[1], b.manos[1])],
        cabeza: l3(a.cabeza, b.cabeza),
        risa: l(a.risa, b.risa),
        llamas: l(a.llamas, b.llamas),
        pataleo: l(a.pataleo, b.pataleo),
        cuerpo: (l(a.cuerpo.0, b.cuerpo.0), l(a.cuerpo.1, b.cuerpo.1)),
        aleteo: l(a.aleteo, b.aleteo),
    }
}

/// El movimiento de este instante, ya mezclado con el anterior.
fn movimiento(p: &SceneParams) -> Mov {
    let edad = p.hada_edad.max(0.0);
    let n = COREOGRAFIA.len();
    let fin = COREOGRAFIA[n - 1].0;
    // Pasado el ultimo, se repiten los de la bendicion.
    let e = if edad > fin + 3.0 {
        let desde = COREOGRAFIA[REPITE_DESDE].0;
        desde + (edad - fin - 3.0) % (fin - desde)
    } else {
        edad
    };
    let i = COREOGRAFIA.iter().rposition(|(t, _)| *t <= e).unwrap_or(0);
    let (t0, ahora) = COREOGRAFIA[i];
    let antes = if i == 0 { ARRIBA } else { COREOGRAFIA[i - 1].1 };
    mezclar(&antes, &ahora, suave((e - t0) / 1.1))
}

/// La pose del hada en este instante de la cancion.
///
/// Lo grande lo decide la coreografia (`movimiento`); lo chico, que esta
/// viva: las manos dibujan curvas lentas con periodos que no coinciden entre
/// si ni con el compas, la cabeza sigue un poco a las manos, respira. Asi
/// nunca repite el mismo gesto y no se mueve al mismo tiempo que Link, que
/// si marca el compas.
fn pose_de(p: &SceneParams) -> Pose {
    let t = p.tiempo;
    let m = movimiento(p);
    let v = |a: (f32, f32, f32)| Vec3::new(a.0, a.1, a.2);
    let vida = 0.6 + 0.4 * p.swell.clamp(0.0, 1.0);
    let gesto = |i: usize| {
        let k = i as f32;
        Vec3::new(
            0.06 * (t * (0.71 + 0.13 * k) + k * 2.1).sin(),
            0.07 * (t * (0.53 + 0.17 * k) + k * 0.7).sin(),
            0.05 * (t * (0.89 - 0.11 * k) + k * 1.3).cos(),
        ) * vida
    };
    // El aleteo: los dos brazos a la vez, lento, bajando rapido y subiendo
    // despacio como un ala.
    let ala = {
        let f = (t * 1.3).sin();
        (f + 0.35 * (t * 2.6).sin()) * 0.28 * m.aleteo
    };
    let manos = [0, 1].map(|i| v(m.manos[i]) + gesto(i) + Vec3::new(0.0, ala, 0.0));
    // La cabeza sigue a la mano de adelante, apenas.
    let mira = (manos[0].x + manos[1].x) * 0.15;
    let risa_viva = m.risa * (0.75 + 0.25 * (t * 1.9).sin());
    Pose {
        tiempo: t,
        manos,
        cabeza: (
            m.cabeza.0 + mira + 0.10 * (t * 0.37).sin(),
            m.cabeza.1 - 0.25 * risa_viva + 0.04 * (t * 0.61).sin(),
            m.cabeza.2 + 0.06 * (t * 0.45).sin(),
        ),
        llamas: m.llamas * (0.7 + 0.3 * p.swell.clamp(0.0, 1.0)),
        patada: t * 2.1,
        pataleo: m.pataleo,
        temblor: m.risa * (t * 31.0).sin(),
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
    let cabeza = raiz.hijo(Vec3::new(0.0, 2.0, 0.0), ejes_de(pose.cabeza.0, pose.cabeza.1, pose.cabeza.2));
    let mut cara = caja(&cabeza, Vec3::new(0.0, 0.16, 0.01), Vec3::new(0.28, 0.33, 0.28), recto, H::Piel);
    cara.cara = true;
    v.push(cara);
    v.push(caja(&cabeza, Vec3::new(0.0, 0.35, -0.01), Vec3::new(0.32, 0.12, 0.32), recto, H::Pelo));
    v.push(caja(&cabeza, Vec3::new(0.0, 0.18, -0.14), Vec3::new(0.32, 0.30, 0.10), recto, H::Pelo));
    for lado in [-1.0f32, 1.0] {
        v.push(caja(&cabeza, Vec3::new(lado * 0.15, 0.20, 0.08), Vec3::new(0.06, 0.24, 0.10), recto, H::Pelo));
    }
    // EL FLEQUILLO EN PICO, como el del modelo del juego: dos mechones que
    // bajan en diagonal desde las sienes y se juntan en punta sobre la
    // frente. Sin el, la cara era un bloque de piel con mucha frente.
    for lado in [-1.0f32, 1.0] {
        v.push(caja(
            &cabeza,
            Vec3::new(lado * 0.075, 0.268, 0.152),
            Vec3::new(0.19, 0.095, 0.035),
            ejes_de(0.0, 0.0, lado * 0.42),
            H::Pelo,
        ));
    }
    // LAS OREJAS en punta, largas, hacia afuera y arriba, como en el juego.
    for lado in [-1.0f32, 1.0] {
        let giro = ejes_de(lado * (PI / 2.0 + 0.25), -0.45, 0.0);
        let hacia = llevar(&giro, &Vec3::new(0.0, 0.0, 1.0));
        let pivote = Vec3::new(lado * 0.14, 0.15, 0.0);
        v.push(caja(&cabeza, pivote + hacia * 0.07, Vec3::new(0.03, 0.08, 0.14), giro, H::Piel));
        v.push(caja(&cabeza, pivote + hacia * 0.17, Vec3::new(0.022, 0.04, 0.09), giro, H::Piel));
    }
    // Y los mechones de los costados, largos, enmarcando la cara.
    for lado in [-1.0f32, 1.0] {
        v.push(caja(
            &cabeza,
            Vec3::new(lado * 0.135, 0.10, 0.13),
            Vec3::new(0.045, 0.24, 0.05),
            ejes_de(0.0, 0.0, -lado * 0.10),
            H::Pelo,
        ));
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
    // LA TERCERA COLETA, como en el juego: del mono, hacia arriba y atras.
    let mut h = cabeza.hijo(Vec3::new(0.0, 0.48, -0.08), ejes_de(0.0, -0.75 - 0.25 * pose.llamas, 0.0));
    for k in 0..5 {
        let ancho = 0.20 - k as f32 * 0.03;
        let largo = 0.25;
        let mat = if k >= 3 { H::PeloPunta } else { H::Pelo };
        v.push(caja(&h, Vec3::new(0.0, largo * 0.5, 0.0), Vec3::new(ancho, largo, ancho * 0.8), recto, mat));
        let onda = (t * 2.2 - k as f32 * 0.8 + 0.5).sin() * 0.2;
        h = h.hijo(Vec3::new(0.0, largo * 0.9, 0.0), ejes_de(onda * 0.6, -0.22 + onda * 0.3, 0.0));
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
        v.push(tramo(hombro - Vec3::new(0.0, 0.02, 0.0), hombro + (c - hombro) * 0.55, 0.13, H::Traje));
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
    let (pa, pb) = (pose.patada.sin() * pose.pataleo, (pose.patada * 0.83 + PI).sin() * pose.pataleo);
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

/// LA HIEDRA que la envuelve, como en el juego: tallos verde oscuro que
/// suben ondulando, con hojitas amarillo verdosas alternadas a cada lado.
/// `tallos` es cuantos hay a lo ancho de la textura. Devuelve el color si en
/// (u, v) hay tallo u hoja.
fn hiedra(u: f32, v: f32, tallos: usize, semilla: u32) -> Option<[f32; 3]> {
    for k in 0..tallos {
        let kf = k as f32;
        let fase = hash(k as u32, semilla) * 6.3;
        let base = (kf + 0.5 + (hash(k as u32, semilla + 1) - 0.5) * 0.5) / tallos as f32;
        let x = base + 0.07 * (v * 2.0 * PI * 1.5 + fase).sin() + 0.03 * (v * 2.0 * PI * 4.0 + fase).sin();
        let dx = ((u - x + 0.5).rem_euclid(1.0)) - 0.5;
        if dx.abs() < 0.014 {
            return Some([0.10, 0.22, 0.07]);
        }
        // Las hojas: cada tanto, alternadas a un lado y al otro del tallo.
        let paso = 0.085;
        let fila = (v / paso).floor();
        let lado = if (fila as i32).rem_euclid(2) == 0 { 1.0 } else { -1.0 };
        let (cx, cy) = (lado * 0.042, (fila + 0.5) * paso);
        let (a, b) = ((dx - cx) / 0.046, (v - cy) / 0.032);
        let d = a * a + b * b;
        if d < 1.0 {
            let borde = if d > 0.65 { 0.55 } else { 1.0 };
            let vena = if (a * 0.6 - b).abs() < 0.12 { 0.8 } else { 1.0 };
            let k = borde * vena;
            return Some([0.78 * k, 0.84 * k, 0.22 * k]);
        }
    }
    None
}

/// La piel del hada del juego: verde amarillenta y palida, con alguna rama
/// de hiedra suelta.
fn textura_piel() -> TextureImage {
    let ruido = campo_fbm(128, 5, 3, 301);
    TextureImage::pintada(128, 128, move |u, v| {
        let n = ruido[((v * 127.0) as usize) * 128 + (u * 127.0) as usize];
        if let Some(c) = hiedra(u, v, 1, 71) {
            return c;
        }
        let k = 0.94 + 0.10 * n;
        [0.80 * k, 0.90 * k, 0.42 * k]
    })
}

/// El "traje": como en el juego es casi piel, un corpino claro cubierto de
/// hiedra tupida.
fn textura_traje() -> TextureImage {
    let ruido = campo_fbm(128, 5, 3, 307);
    TextureImage::pintada(128, 128, move |u, v| {
        let n = ruido[((v * 127.0) as usize) * 128 + (u * 127.0) as usize];
        if let Some(c) = hiedra(u, v, 4, 13) {
            return c;
        }
        let k = 0.92 + 0.10 * n;
        [0.86 * k, 0.90 * k, 0.50 * k]
    })
}

/// El pelo: rojo carmesi, con mechones y hiedra enredada; las puntas, mas
/// claras, como una llama.
fn textura_pelo(punta: bool) -> TextureImage {
    let ruido = campo_fbm(64, 4, 3, 311);
    TextureImage::pintada(64, 64, move |u, v| {
        let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
        if !punta {
            if let Some(c) = hiedra(u, v, 1, 29) {
                return c;
            }
        }
        let veta = 0.75 + 0.25 * ((u * 36.0 + n * 7.0).sin() * 0.5 + 0.5);
        if punta {
            [1.0 * veta, (0.30 + 0.15 * v) * veta, (0.32 + 0.1 * v) * veta]
        } else {
            [0.86 * veta, 0.10 * veta, 0.18 * veta]
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

/// LA CARA del Hada Mayor de Ocarina, sacada de la captura del juego: piel
/// verde amarillenta, ojos grandes mirando de reojo con el iris violeta,
/// sombra ROJA muy marcada hasta unas cejas rojas y gruesas, labios morados
/// en una sonrisa, y la barbilla en punta (la cabeza es un cubo: la punta
/// se hace con sombra en las esquinas de abajo). Se pinta en coordenadas
/// corregidas por la proporcion de la cabeza, para que lo redondo salga
/// redondo.
fn textura_cara() -> TextureImage {
    const ASPECTO: f32 = 0.33 / 0.28;
    TextureImage::pintada(256, 256, |u, v| {
        let y = v * ASPECTO;
        let mezcla = |c: [f32; 3], d: [f32; 3], k: f32| {
            let k = k.clamp(0.0, 1.0);
            [c[0] + (d[0] - c[0]) * k, c[1] + (d[1] - c[1]) * k, c[2] + (d[2] - c[2]) * k]
        };
        let lado = 1.0 - 0.18 * ((u - 0.5) * 2.0).powi(4);
        let mut c = [0.80 * lado, 0.90 * lado, 0.42 * lado];

        // LA BARBILLA EN PUNTA: debajo de la boca, las esquinas se hunden en
        // sombra y queda una V de piel.
        let v_menton = (u - 0.5).abs() - (1.0 - v) * 1.35;
        if v > 0.70 && v_menton > 0.0 {
            c = mezcla(c, [0.30, 0.36, 0.16], (v_menton / 0.06).min(1.0) * 0.85);
        }

        let (ojo_y, ancho, alto) = (0.64f32, 0.105f32, 0.070f32);
        for (cx, s) in [(0.30f32, -1.0f32), (0.70, 1.0)] {
            // La sombra roja, grande, del ojo a la ceja, esfumada.
            {
                let dx = (u - cx - s * 0.01) / 0.15;
                let dy = (y - (ojo_y - 0.06)) / 0.085;
                let d = dx * dx + dy * dy;
                if d < 1.0 && y < ojo_y + 0.01 {
                    c = mezcla(c, [0.84, 0.10, 0.20], (1.0 - d).powf(0.45) * 1.1);
                }
            }
            let x = (u - cx) / ancho * s;
            if x.abs() < 1.0 {
                let curva = (1.0 - x * x).max(0.0);
                let arriba = ojo_y - alto * curva.powf(0.55) - 0.010 * x.max(0.0);
                let abajo = ojo_y + alto * 0.75 * curva - 0.010 * x.max(0.0);
                if y > arriba && y < abajo {
                    c = [0.97, 0.97, 0.94];
                    // Mira de reojo: el iris corrido hacia la derecha de ella.
                    let (ix, iy) = ((u - cx - 0.035) / 0.050, (y - ojo_y - 0.004) / 0.056);
                    let ri = ix * ix + iy * iy;
                    if ri < 1.0 {
                        let k = 0.6 + 0.5 * (iy * 0.5 + 0.5);
                        c = [0.42 * k, 0.20 * k, 0.55 * k];
                        if ri < 0.25 {
                            c = [0.05, 0.02, 0.06];
                        }
                        let b1 = ((u - cx - 0.020) / 0.013).powi(2) + ((y - ojo_y + 0.020) / 0.013).powi(2);
                        if b1 < 1.0 {
                            c = [1.0, 1.0, 1.0];
                        }
                    }
                }
                // El delineado de arriba, grueso, y la linea de abajo.
                if y <= arriba && y > arriba - 0.012 {
                    c = [0.10, 0.03, 0.06];
                }
                if y >= abajo && y < abajo + 0.005 {
                    c = mezcla(c, [0.35, 0.10, 0.15], 0.8);
                }
            }
            // La ceja: roja, gruesa, alta y arqueada.
            let x = (u - cx) / 0.13;
            let ey = y - (ojo_y - 0.150 - 0.030 * (1.0 - x * x) + s * x * 0.015);
            if x.abs() < 1.0 && ey.abs() < 0.016 * (1.0 - 0.6 * x.abs()) {
                c = [0.72, 0.06, 0.14];
            }
        }

        // La nariz: una sombra larga y fina.
        if (u - 0.5).abs() < 0.012 && (0.70..0.80).contains(&y) {
            c = mezcla(c, [0.55, 0.62, 0.32], 0.5);
        }

        // LOS LABIOS, morados, en una sonrisa cerrada.
        {
            let x = (u - 0.5) / 0.085;
            let medio = 0.885 - 0.014 * x * x;
            if x.abs() < 1.0 {
                let arco = 0.020 * (1.0 - x * x).powf(0.7);
                let abajo = 0.026 * (1.0 - x * x).powf(0.8);
                if y > medio - arco && y < medio + abajo {
                    c = [0.62, 0.12, 0.44];
                    let brillo = ((u - 0.51) / 0.025).powi(2) + ((y - medio - abajo * 0.45) / 0.007).powi(2);
                    if brillo < 1.0 {
                        c = mezcla(c, [0.95, 0.70, 0.90], 1.0 - brillo);
                    }
                    if (y - medio).abs() < 0.003 {
                        c = [0.32, 0.04, 0.22];
                    }
                }
            }
        }
        c
    })
}

/// Arma a la Gran Hada (escondida: aparece con la cancion) y su luz.
pub fn armar(objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>, luces: &mut Vec<Light>) -> HadaMayorViva {
    let img = |t: TextureImage| Texture::ImageTexture(Arc::new(t), Color::WHITE, (0.0, 0.0));
    let mats = [
        // La piel brilla apenas: es un ser de luz, no de carne.
        Material::new([0.95, 0.06, 0.0, 0.0], 12.0, 0.0, img(textura_piel()), Some(Color::new(10, 12, 4, 255))),
        Material::new([1.0, 0.30, 0.0, 0.0], 30.0, 0.0, img(textura_traje()), Some(Color::new(8, 10, 3, 255))),
        // El pelo brilla apenas: es lo que la hace leerse magica y no un
        // maniqui, y lo que mas halo le saca al bloom.
        Material::new([1.0, 0.5, 0.0, 0.0], 40.0, 0.0, img(textura_pelo(false)), Some(Color::new(55, 5, 12, 255))),
        Material::new([1.0, 0.2, 0.0, 0.0], 20.0, 0.0, img(textura_botas()), None),
        Material::new([1.0, 0.5, 0.0, 0.0], 40.0, 0.0, img(textura_pelo(true)), Some(Color::new(80, 18, 22, 255))),
    ];
    let cara = Material::new([0.9, 0.05, 0.0, 0.0], 12.0, 0.0, img(textura_cara()), Some(Color::new(10, 12, 4, 255)));

    let reposo = Pose {
        tiempo: 0.0,
        manos: [Vec3::new(-0.44, 1.32, 0.3), Vec3::new(0.3, 2.5, -0.05)],
        cabeza: (0.0, 0.0, 0.0),
        llamas: 0.0,
        patada: 0.0,
        pataleo: 0.0,
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
    luces.push(Light::new(FLOTA + Vec3::new(0.0, 0.4, 1.4), Color::new(255, 160, 220, 255), 0.0).con_alcance(2.4));

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
    // EL RAYO DE LA BENDICION: de CADA mano del hada baja un haz dorado a la
    // mano levantada de Link del mismo lado, con un nucleo fino y un halo
    // ancho (translucidos: suman su luz, no tapan), y en cada mano del hada
    // una esfera de luz. Orden: nucleo 0, halo 0, nucleo 1, halo 1, luz de
    // la mano 0, luz de la mano 1.
    let velo = Material::new([0.0, 0.0, 0.0, 1.0], 1.0, 1.0, Texture::Solid(Color::WHITE), Some(Color::BLACK));
    let mut rayo_partes: Vec<Box<dyn RayIntersect + Send + Sync>> = Vec::new();
    for _ in 0..4 {
        let mut haz = crate::cylinder::CilindroOrientado::nuevo(FLOTA, FLOTA + Vec3::new(0.0, 0.1, 0.0), 0.0, 1.0, velo.clone());
        haz.set_visible(false);
        rayo_partes.push(Box::new(haz));
    }
    for _ in 0..2 {
        rayo_partes.push(Box::new(crate::sphere::Sphere { center: FLOTA, radius: 0.0, material: velo.clone() }));
    }
    let rayo = objetos.len();
    objetos.push(Box::new(GrupoAcotado::new(rayo_partes)));
    HadaMayorViva { grupo, luz, chispas, luz_link, rayo }
}

/// Las manos levantadas de Link, cada una emparejada con la mano del hada
/// que le queda del mismo lado (asi los haces no se cruzan).
fn manos_de_link(manos_hada: &[Vec3; 2]) -> [Vec3; 2] {
    let izq = crate::link::PIES + Vec3::new(-0.2, 1.68, -0.10);
    let der = crate::link::PIES + Vec3::new(0.2, 1.68, -0.10);
    if manos_hada[0].x < manos_hada[1].x {
        [izq, der]
    } else {
        [der, izq]
    }
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
    let escala = (0.12 + 0.88 * suave(presencia)) * TAMANIO;
    let giro = (1.0 - suave(presencia)) * 3.0 * PI;
    // LA VOLTERETA: mientras sale del agua da una vuelta entera hacia atras,
    // ademas de girar; se completa justo cuando llega arriba.
    let voltereta = (1.0 - suave((presencia - 0.15) / 0.55)) * 2.0 * PI * (presencia > 0.02) as i32 as f32;
    // Flota, y ademas BAILA a su aire: se mece y deriva en un ocho lento,
    // con periodos que no son los del compas (el compas lo marca Link), y
    // sube cuando la cancion crece.
    let vaiven = (t * 0.83).sin() * (0.35 + 0.65 * p.energia_suave.clamp(0.0, 1.0));
    let flota = Vec3::new(
        0.12 * vaiven + 0.16 * (t * 0.41).sin(),
        0.16 * (t * 0.9).sin() + 0.06 * (t * 2.3).sin() + 0.08 * p.swell.clamp(0.0, 1.0),
        0.05 * (t * 0.7).cos() + 0.07 * (t * 0.82).sin(),
    ) * sube;
    let m = movimiento(p);
    let base = Vec3::new(0.0, crate::fuente::CUENCO_Y - 1.2, 0.0);
    // Arriba del cuenco, y para bendecir avanza sobre Link.
    let arriba = FLOTA + (BENDICE - FLOTA) * b;
    let donde = base + (arriba - base) * sube + flota;
    // DE PIE, FLOTANDO. Estuvo recostada de costado, como en una toma de la
    // cinematica, pero quieta en esa pose se leia rara: ahora flota derecha,
    // con las piernas colgando, y lo unico que la inclina es el baile.
    let r = ejes_de(
        giro + m.cuerpo.0 + (0.10 * (t * 0.5).sin() + 0.18 * vaiven) * (1.0 - b),
        -0.12 * b + 0.05 * (t * 0.6).sin() - voltereta,
        m.cuerpo.1 + 0.07 * (t * 0.8).sin() + 0.08 * vaiven,
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
                            Some(Color::new((70.0 * k) as u8, (6.0 * k) as u8, (14.0 * k) as u8, 255));
                    }
                }
                }
                parte.recalcular_caja(0.0);
            }
            g.recalcular_caja(0.0);
        }

        // La luz rosa del centro de la fuente (la 1, ver `main.rs`) queda
        // adentro de su cuerpo cuando esta afuera y le quemaba la piel: sube
        // por encima de su cabeza mientras flota.
        if let Some(luz) = luces.get_mut(1) {
            luz.position = Vec3::new(0.0, 3.0 + 2.8 * suave(presencia), 0.0);
        }
        // Su luz: rosa, delante de ella, mirandola. Es la que la saca del
        // contraluz de la fuente.
        if let Some(luz) = luces.get_mut(self.luz) {
            luz.position = donde + Vec3::new(0.0, 0.9, 2.2);
            luz.intensity = 1.6 * suave(presencia) * (0.85 + 0.3 * p.pulso);
        }
        // La luz dorada sobre Link mientras recibe el poder.
        if let Some(luz) = luces.get_mut(self.luz_link) {
            luz.intensity = 2.2 * p.bendicion * (0.8 + 0.4 * p.pulso);
        }

        // ---- EL RAYO ----
        // De cada mano del hada a la mano de Link del mismo lado (las de
        // Link estan arriba, abiertas: ver `link::pose_de`). Siguen a las
        // manos del hada por toda la coreografia.
        let manos_hada = [al_mundo(pose.manos[0]), al_mundo(pose.manos[1])];
        let destinos = manos_de_link(&manos_hada);
        if let Some(g) = objetos
            .get_mut(self.rayo)
            .and_then(|o| (o.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>())
        {
            let b = p.bendicion;
            for (i, hijo) in g.children_mut().iter_mut().enumerate() {
                if let Some(c) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<crate::cylinder::CilindroOrientado>() {
                    c.set_visible(b > 0.05);
                    if b > 0.05 {
                        let mano = i / 2;
                        let halo = i % 2 == 1;
                        // El haz respira, cada mano a su tiempo.
                        let respira = 0.85 + 0.15 * (t * 2.3 + mano as f32 * 1.9).sin();
                        c.recolocar(destinos[mano], manos_hada[mano]);
                        c.set_radio(if halo { 0.17 } else { 0.05 } * b * respira);
                        let k = b * respira * if halo { 0.16 } else { 0.7 };
                        c.material_mut().emission_color =
                            Some(Color::new((255.0 * k) as u8, (215.0 * k) as u8, (130.0 * k) as u8, 255));
                    }
                } else if let Some(e) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<crate::sphere::Sphere>() {
                    let mano = i - 4;
                    let late = 0.85 + 0.15 * (t * 3.1 + mano as f32).sin();
                    e.center = manos_hada[mano.min(1)];
                    e.radius = if b > 0.05 { 0.16 * b * late } else { 0.0 };
                    let k = b * 0.5;
                    e.material.emission_color = Some(Color::new((255.0 * k) as u8, (225.0 * k) as u8, (160.0 * k) as u8, 255));
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
        let manos = manos_hada;
        let manos_link = destinos;
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
                // Cada chispa baja por uno de los dos haces, de la mano del
                // hada a la de Link, girando en espiral alrededor del haz,
                // desfasada de las demas: el poder corre por el rayo.
                let s = (t * 0.7 + k as f32 / CHISPAS as f32).fract();
                let lado = k % 2;
                let (desde, hasta) = (manos[lado], manos_link[lado]);
                let eje = normalize(&(hasta - desde));
                let a = normalize(&cross(&eje, &Vec3::new(0.0, 1.0, 0.0)));
                let b = cross(&eje, &a);
                let vuelta = s * 3.0 * PI + t * 4.0 + h(4) * 6.0;
                let radio = 0.09 * (s * PI).sin();
                let pos = desde + (hasta - desde) * s + (a * vuelta.cos() + b * vuelta.sin()) * radio;
                let brillo = (s * PI).sin();
                (pos, 0.06 * brillo * p.bendicion, (255.0, 225.0, 150.0))
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
