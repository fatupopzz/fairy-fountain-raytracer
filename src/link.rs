//! LINK, el de Ocarina of Time, hecho de cubos.
//!
//! Esta parado en el escalon de la entrada, de espaldas a la camara y de
//! frente a la fuente, tocando la ocarina: es la escena del juego en la que
//! Link toca la cancion de Zelda para que salga el Hada Mayor. Tiene la
//! tunica verde, el gorro largo que le cae por la espalda, las orejas de
//! hylian, el escudo hyliano y la Espada Maestra cruzados en la espalda, y
//! la Ocarina del Tiempo en la boca.
//!
//! Todo son `CajaOrientada`: cubos texturizados que se pueden girar. No hay
//! un solo archivo de modelo; la forma sale de este codigo y las texturas
//! (la cara, la tela, el escudo) se pintan pixel por pixel al arrancar.
//!
//! EL ESQUELETO. Las piezas no se colocan en el mundo sino colgadas de
//! HUESOS: la raiz en los pies, el torso en la cadera, la cabeza en el
//! cuello, y el gorro como una cadena de cinco eslabones que cuelga de la
//! coronilla. Cada hueso es un origen y tres ejes relativos a su padre, asi
//! que girar el torso arrastra la cabeza, los brazos, el escudo y el gorro
//! sin tocar una sola pieza a mano. Los brazos no se posan con angulos: se
//! resuelven con cinematica inversa de dos huesos para que las manos
//! queden SIEMPRE sobre la ocarina, se mueva lo que se mueva la cabeza.
//!
//! LA POSE SE RECALCULA EN CADA CUADRO a partir de la cancion: se balancea
//! con el compas, asiente en cada tiempo, respira, y el gorro llega tarde a
//! cada movimiento como la tela de verdad. Ver `pose_de`.

use crate::caja_orientada::{componer, ejes_de, llevar, CajaOrientada};
use crate::grupo_acotado::GrupoAcotado;
use crate::material::Material;
use crate::ray_intersect::RayIntersect;
use crate::sync::SceneParams;
use crate::texture::{campo_fbm, Texture, TextureImage};
use crate::vec3::{cross, normalize, Vec3};
use raylib::prelude::Color;
use std::any::Any;
use std::f32::consts::PI;
use std::sync::Arc;

/// Donde estan los pies de Link: en el pasillo de baldosas, justo detras de
/// la Trifuerza del piso y de frente al estrado, que es donde se para en el
/// juego para tocar la cancion.
pub const PIES: Vec3 = Vec3::new(0.0, 0.345, 3.0);

/// Hacia donde mira: a la Trifuerza del altar.
pub fn guinada_base() -> f32 {
    (0.0 - PIES.x).atan2(0.0 - PIES.z)
}

/// Donde esta la boca de Link (y la ocarina) en reposo, en el mundo. Es de
/// donde salen las notas.
pub fn boca() -> Vec3 {
    let pose = Pose::quieta();
    let esqueleto = Esqueleto::de(&pose);
    esqueleto.cabeza.punto(OCARINA_EN_CABEZA)
}

/// Donde queda la ocarina respecto del hueso de la cabeza.
const OCARINA_EN_CABEZA: Vec3 = Vec3::new(0.0, 0.07, 0.21);

/// Los materiales de Link, por nombre. El orden es el del arreglo que arma
/// `materiales`.
#[derive(Clone, Copy)]
enum M {
    Tunica,
    Piel,
    Pelo,
    Blanco,
    Cuero,
    CueroOscuro,
    Oro,
    EscudoCanto,
    Ocarina,
    Empunadura,
    Vaina,
}

/// Una pieza ya colocada: centro, ejes y tamano en el mundo, y su material.
struct Pieza {
    centro: Vec3,
    ejes: [Vec3; 3],
    tam: Vec3,
    mat: M,
    /// Si la cara +Z lleva otro material (la cara, el frente del escudo).
    frente: Option<Frente>,
}

#[derive(Clone, Copy)]
enum Frente {
    Cara,
    Escudo,
    EscudoPunta,
}

/// Un hueso: un origen y tres ejes, en el mundo.
#[derive(Clone, Copy)]
pub(crate) struct Hueso {
    pub(crate) o: Vec3,
    pub(crate) e: [Vec3; 3],
}

impl Hueso {
    pub(crate) fn hijo(&self, pivote: Vec3, rot: [Vec3; 3]) -> Hueso {
        Hueso {
            o: self.punto(pivote),
            e: componer(&self.e, &rot),
        }
    }

    pub(crate) fn punto(&self, p: Vec3) -> Vec3 {
        self.o + llevar(&self.e, &p)
    }

    fn caja(&self, centro: Vec3, tam: Vec3, rot: [Vec3; 3], mat: M) -> Pieza {
        Pieza {
            centro: self.punto(centro),
            ejes: componer(&self.e, &rot),
            tam,
            mat,
            frente: None,
        }
    }

    fn recta(&self, centro: Vec3, tam: Vec3, mat: M) -> Pieza {
        self.caja(centro, tam, ejes_de(0.0, 0.0, 0.0), mat)
    }
}

/// Una caja estirada de `a` a `b` (en el mundo), con su eje Y a lo largo
/// del segmento y el X lo mas parecido posible a `lado`. Es lo que se usa
/// para los brazos y la espada, que se definen por sus extremos y no por
/// angulos.
fn segmento(a: Vec3, b: Vec3, grueso: f32, ancho: f32, lado: Vec3, mat: M) -> Pieza {
    let largo = (b - a).norm().max(1e-4);
    let y = (b - a) / largo;
    let mut z = cross(&lado, &y);
    if z.norm() < 1e-4 {
        z = cross(&Vec3::new(0.0, 0.0, 1.0), &y);
    }
    let z = normalize(&z);
    let x = cross(&y, &z);
    Pieza {
        centro: (a + b) * 0.5,
        ejes: [x, y, z],
        tam: Vec3::new(ancho, largo, grueso),
        mat,
        frente: None,
    }
}

/// Cinematica inversa de dos huesos: el codo de un brazo que va del hombro
/// `s` a la mano `h`, con los dos tramos de largo `l1` y `l2`, doblado hacia
/// `polo`. Si la mano quedo mas lejos de lo que el brazo alcanza, se estira
/// derecho hacia ella.
pub(crate) fn codo(s: Vec3, h: Vec3, l1: f32, l2: f32, polo: Vec3) -> Vec3 {
    let d = h - s;
    let dist = d.norm().clamp(1e-3, l1 + l2 - 1e-3);
    let eje = d / d.norm().max(1e-4);
    // Ley de cosenos: a que distancia del hombro, a lo largo del eje, cae
    // la proyeccion del codo, y a que altura sobre el eje queda.
    let a = (l1 * l1 - l2 * l2 + dist * dist) / (2.0 * dist);
    let alto = (l1 * l1 - a * a).max(0.0).sqrt();
    let p = polo - eje * crate::vec3::dot(&polo, &eje);
    let p = if p.norm() > 1e-4 { normalize(&p) } else { Vec3::new(1.0, 0.0, 0.0) };
    s + eje * a + p * alto
}

/// Lo que se mueve de Link, cuadro a cuadro.
#[derive(Clone, Copy)]
pub struct Pose {
    /// Balanceo lateral del cuerpo entero, en radianes (alrededor de Z).
    balanceo: f32,
    /// Cuanto se inclina hacia adelante al tocar.
    inclinacion: f32,
    /// Cabeceo extra de la cabeza: el asentir con el tiempo.
    asiente: f32,
    /// Subida del pecho al respirar.
    respira: f32,
    /// El gorro: cuanto se levanta y cuanto se va de costado.
    gorro_arriba: f32,
    gorro_lado: f32,
    /// Fase del segundo, para el aleteo fino del gorro.
    tiempo: f32,
    /// Cuanto esta recibiendo la bendicion del hada (0 a 1): guarda la
    /// ocarina, levanta los dos brazos y mira hacia arriba.
    recibe: f32,
    /// Cuanto baja la cadera, en metros: las rodillas se doblan (con
    /// cinematica inversa, como los brazos) y los pies quedan en el piso.
    baja: f32,
    /// Giro del torso sobre la cadera, en radianes.
    gira: f32,
    /// El Fuego de Din: `prepara` levanta los punos (0 a 1) y `azota` baja
    /// el izquierdo contra el piso.
    prepara: f32,
    azota: f32,
}

impl Pose {
    pub fn quieta() -> Pose {
        Pose {
            balanceo: 0.0,
            inclinacion: 0.0,
            asiente: 0.0,
            respira: 0.0,
            gorro_arriba: 0.0,
            gorro_lado: 0.0,
            tiempo: 0.0,
            recibe: 0.0,
            baja: 0.0,
            gira: 0.0,
            prepara: 0.0,
            azota: 0.0,
        }
    }
}

/// La pose que pide la cancion en este instante.
///
/// Todo es funcion del segundo (como el resto de la escena, ver
/// `animacion.rs`), nada se acumula entre cuadros. El balanceo va a media
/// velocidad del compas (un vaiven cada dos tiempos, que es como se mece
/// alguien que toca), el cabeceo es el golpe de cada tiempo y la respiracion
/// va por su cuenta, lenta. El gorro sigue al balanceo con retraso: su
/// fase va atrasada un cuarto de ciclo, que es lo que hace que parezca
/// tela colgando y no una pieza pegada.
pub fn pose_de(p: &SceneParams) -> Pose {
    let beat = p.beat_period.max(0.2);
    let fase = p.tiempo / (beat * 2.0) * 2.0 * PI;
    let energia = p.energia_suave.clamp(0.0, 1.0);
    let vaiven = 0.035 + 0.045 * energia + 0.03 * p.swell;

    // EL FUEGO DE DIN, como en el juego: se agacha, levanta los punos y
    // golpea el piso con el izquierdo en el instante en que la cupula se
    // abre en el centro de la fuente (ver `fuego_de_din.rs`).
    let e = p.hechizo;
    let (prepara, azota) = if e >= 0.0 {
        let suave = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let se_levanta = 1.0 - suave((e - 2.4) / 0.7);
        let golpe = crate::fuego_de_din::GOLPE;
        (suave(e / 0.35) * se_levanta, suave((e - golpe + 0.1) / 0.1) * se_levanta)
    } else {
        (0.0, 0.0)
    };
    // Mientras el hada esta afuera (y antes de la bendicion) la mira.
    let mira = p.hada.clamp(0.0, 1.0) * (1.0 - p.bendicion.clamp(0.0, 1.0)) * 0.22;
    Pose {
        balanceo: fase.sin() * vaiven * (1.0 - azota),
        inclinacion: 0.06 + 0.05 * energia + 0.06 * p.swell + 0.45 * azota,
        asiente: p.pulso * 0.09 - 0.02 - mira - 0.3 * prepara * (1.0 - azota) + 0.25 * azota,
        respira: (p.tiempo * 1.4).sin() * 0.008,
        gorro_arriba: 0.10 * p.pulso + 0.05 * p.swell + 0.35 * azota,
        gorro_lado: -(fase - PI / 2.0).sin() * vaiven * 2.2,
        tiempo: p.tiempo,
        recibe: p.bendicion.clamp(0.0, 1.0),
        // Marca el tiempo con las rodillas, y se agacha para el hechizo.
        baja: 0.03 * p.pulso + 0.015 * p.swell + 0.06 * prepara + 0.15 * azota,
        // Y el torso acompana el vaiven, a contratiempo de la cadera.
        gira: (fase * 0.5).sin() * (0.05 + 0.08 * energia) * (1.0 - prepara),
        prepara,
        azota,
    }
}

/// Los huesos que importan fuera de `piezas`.
struct Esqueleto {
    raiz: Hueso,
    cuerpo: Hueso,
    torso: Hueso,
    cabeza: Hueso,
}

impl Esqueleto {
    fn de(pose: &Pose) -> Esqueleto {
        let raiz = Hueso {
            o: PIES,
            e: ejes_de(guinada_base(), 0.0, pose.balanceo),
        };
        // La cadera: baja cuando se doblan las rodillas y gira un poco.
        let cuerpo = raiz.hijo(Vec3::new(0.0, -pose.baja, 0.0), ejes_de(-pose.gira * 0.4, 0.0, 0.0));
        let torso = cuerpo.hijo(
            Vec3::new(0.0, 0.78 + pose.respira, 0.0),
            ejes_de(pose.gira, pose.inclinacion, pose.balanceo * 0.6),
        );
        let cabeza = torso.hijo(
            Vec3::new(0.0, 0.54, 0.0),
            ejes_de(0.0, 0.10 + pose.asiente - 0.55 * pose.recibe, -pose.balanceo * 1.2),
        );
        Esqueleto { raiz, cuerpo, torso, cabeza }
    }
}

/// TODAS las piezas de Link en la pose dada, siempre en el mismo orden: la
/// primera vez se usan para crear las cajas y despues solo para moverlas.
fn piezas(pose: &Pose) -> (Vec<Pieza>, Vec<usize>) {
    let mut v: Vec<Pieza> = Vec::with_capacity(64);
    // Donde termina cada PARTE del cuerpo: cada una va en su propio grupo
    // acotado, asi que un rayo que pasa por la cabeza no prueba las botas.
    let mut cortes: Vec<usize> = Vec::new();
    let Esqueleto { raiz, cuerpo, torso, cabeza } = Esqueleto::de(pose);

    // ---- PIERNAS Y BOTAS ----
    // Las botas marrones con el borde doblado arriba, y las calzas
    // blancas de adulto.
    for lado in [-1.0f32, 1.0] {
        let x = lado * 0.09;
        v.push(raiz.caja(
            Vec3::new(x, 0.13, 0.025),
            Vec3::new(0.15, 0.26, 0.23),
            ejes_de(lado * 0.12, 0.0, 0.0),
            M::Cuero,
        ));
        v.push(raiz.recta(Vec3::new(x, 0.27, 0.0), Vec3::new(0.17, 0.05, 0.19), M::CueroOscuro));
        // Muslo y pierna, de la cadera al tobillo: la rodilla la resuelve
        // `codo`, hacia adelante y apenas hacia afuera.
        let cadera = cuerpo.punto(Vec3::new(x, 0.64, 0.0));
        let tobillo = raiz.punto(Vec3::new(x, 0.29, 0.0));
        let polo = raiz.e[2] + raiz.e[0] * lado * 0.3;
        let rodilla = codo(cadera, tobillo, 0.18, 0.18, polo);
        v.push(segmento(cadera, rodilla, 0.13, 0.125, raiz.e[0], M::Blanco));
        v.push(segmento(rodilla, tobillo - raiz.e[1] * 0.02, 0.12, 0.115, raiz.e[0], M::Blanco));
    }

    // ---- FALDON DE LA TUNICA ----
    // Dos cajas: el cuerpo y el ruedo, un poco mas ancho, que es lo que le
    // da la forma acampanada.
    v.push(cuerpo.recta(Vec3::new(0.0, 0.69, 0.0), Vec3::new(0.34, 0.24, 0.25), M::Tunica));
    v.push(cuerpo.recta(Vec3::new(0.0, 0.59, 0.0), Vec3::new(0.41, 0.08, 0.29), M::Tunica));

    cortes.push(v.len());
    // ---- TORSO ----
    v.push(torso.recta(Vec3::new(0.0, 0.26, 0.0), Vec3::new(0.35, 0.42, 0.22), M::Tunica));
    // El cinturon, con la hebilla de oro adelante.
    v.push(torso.recta(Vec3::new(0.0, 0.07, 0.0), Vec3::new(0.37, 0.07, 0.245), M::CueroOscuro));
    v.push(torso.recta(Vec3::new(0.0, 0.07, 0.125), Vec3::new(0.08, 0.06, 0.02), M::Oro));
    // La correa del escudo cruzando el pecho en diagonal.
    v.push(torso.caja(
        Vec3::new(0.0, 0.28, 0.0),
        Vec3::new(0.05, 0.52, 0.232),
        ejes_de(0.0, 0.0, 0.62),
        M::Cuero,
    ));
    // El cuello de la camisa blanca y el cuello.
    v.push(torso.recta(Vec3::new(0.0, 0.47, 0.0), Vec3::new(0.18, 0.04, 0.16), M::Blanco));
    v.push(torso.recta(Vec3::new(0.0, 0.52, 0.0), Vec3::new(0.10, 0.08, 0.10), M::Piel));

    cortes.push(v.len());
    // ---- CABEZA ----
    let mut cara = cabeza.recta(Vec3::new(0.0, 0.14, 0.01), Vec3::new(0.25, 0.27, 0.25), M::Piel);
    cara.frente = Some(Frente::Cara);
    v.push(cara);
    // La nariz, apenas: la de Link adulto es fina y en punta.
    v.push(cabeza.caja(
        Vec3::new(0.0, 0.115, 0.14),
        Vec3::new(0.035, 0.05, 0.04),
        ejes_de(0.0, -0.3, 0.0),
        M::Piel,
    ));
    // El pelo rubio: flequillo, dos mechones al costado de la cara y la
    // nuca.
    v.push(cabeza.recta(Vec3::new(0.0, 0.245, 0.10), Vec3::new(0.27, 0.07, 0.08), M::Pelo));
    for lado in [-1.0f32, 1.0] {
        v.push(cabeza.caja(
            Vec3::new(lado * 0.115, 0.16, 0.10),
            Vec3::new(0.055, 0.17, 0.07),
            ejes_de(0.0, 0.0, lado * 0.12),
            M::Pelo,
        ));
    }
    v.push(cabeza.recta(Vec3::new(0.0, 0.16, -0.10), Vec3::new(0.27, 0.22, 0.08), M::Pelo));

    // LAS OREJAS DE HYLIAN: largas, en punta, hacia afuera y atras, y
    // apenas levantadas.
    for lado in [-1.0f32, 1.0] {
        let ejes = ejes_de(lado * (PI / 2.0 + 0.45), -0.32, 0.0);
        let pivote = Vec3::new(lado * 0.12, 0.13, -0.01);
        let base = cabeza.punto(pivote);
        let ejes_mundo = componer(&cabeza.e, &ejes);
        v.push(Pieza {
            centro: base + ejes_mundo[2] * 0.06,
            ejes: ejes_mundo,
            tam: Vec3::new(0.03, 0.065, 0.12),
            mat: M::Piel,
            frente: None,
        });
        v.push(Pieza {
            centro: base + ejes_mundo[2] * 0.14 + ejes_mundo[1] * 0.004,
            ejes: ejes_mundo,
            tam: Vec3::new(0.025, 0.035, 0.07),
            mat: M::Piel,
            frente: None,
        });
    }

    // ---- EL GORRO ----
    // Una banda que abraza la cabeza y una cadena de eslabones cada vez mas
    // finos que se va doblando hacia atras y hacia abajo. Cada eslabon se
    // cuelga del anterior, asi que el ondear se suma a lo largo de la
    // cadena y la punta es la que mas se mueve.
    v.push(cabeza.caja(
        Vec3::new(0.0, 0.285, -0.015),
        Vec3::new(0.285, 0.08, 0.285),
        ejes_de(0.0, -0.08, 0.0),
        M::Tunica,
    ));
    let mut eslabon = cabeza.hijo(Vec3::new(0.0, 0.29, -0.06), ejes_de(0.0, -0.45 + pose.gorro_arriba, 0.0));
    const GORRO: [(f32, f32, f32); 5] = [
        (0.25, 0.11, 0.15),
        (0.20, 0.09, 0.14),
        (0.15, 0.075, 0.13),
        (0.105, 0.06, 0.12),
        (0.06, 0.045, 0.11),
    ];
    for (k, (ancho, alto, largo)) in GORRO.into_iter().enumerate() {
        v.push(eslabon.recta(Vec3::new(0.0, 0.0, -largo * 0.5), Vec3::new(ancho, alto, largo), M::Tunica));
        let k = k as f32;
        let ondea = (pose.tiempo * 2.3 - k * 0.7).sin() * 0.05 * (k + 1.0) * 0.4;
        eslabon = eslabon.hijo(
            Vec3::new(0.0, 0.0, -largo * 0.92),
            ejes_de(
                pose.gorro_lado * 0.35 + ondea * 0.6,
                -0.28 + pose.gorro_arriba * 0.6 + ondea,
                0.0,
            ),
        );
    }

    cortes.push(v.len());
    // ---- LA OCARINA DEL TIEMPO ----
    // Azul, en la boca, cruzada. Cuelga de la cabeza: si Link asiente, la
    // ocarina asiente con el.
    let ocarina = cabeza.hijo(OCARINA_EN_CABEZA, ejes_de(0.0, 0.25, 0.0));
    // Para recibir la bendicion la guarda: se achica hasta desaparecer.
    let guardada = 1.0 - (pose.recibe.max(pose.prepara) * 2.0).clamp(0.0, 1.0);
    v.push(ocarina.recta(Vec3::zeros(), Vec3::new(0.16, 0.065, 0.075) * guardada, M::Ocarina));
    v.push(ocarina.recta(Vec3::new(0.0, 0.0, -0.05), Vec3::new(0.035, 0.03, 0.05) * guardada, M::Ocarina));
    v.push(ocarina.recta(Vec3::new(0.0, 0.034, 0.0), Vec3::new(0.10, 0.01, 0.05) * guardada, M::Oro));

    // ---- BRAZOS ----
    // Los hombros estan en el torso; las manos, en los extremos de la
    // ocarina. El codo lo resuelve `codo`, abierto hacia afuera y abajo,
    // que es como se toca una ocarina transversal.
    let adelante = torso.e[2];
    for lado in [-1.0f32, 1.0] {
        let hombro = torso.punto(Vec3::new(lado * 0.215, 0.43, 0.0));
        // Tocando, las manos van a los extremos de la ocarina; recibiendo la
        // bendicion, arriba de la cabeza, abiertas hacia el hada.
        let en_ocarina = ocarina.punto(Vec3::new(lado * 0.085, -0.02, 0.0));
        let arriba = torso.punto(Vec3::new(lado * 0.2, 0.88, 0.10));
        let mano = en_ocarina + (arriba - en_ocarina) * pose.recibe;
        // El Fuego de Din: los dos punos arriba, y el izquierdo (+X, Link
        // es zurdo) baja de golpe al piso, delante de los pies.
        let puno = torso.punto(Vec3::new(lado * 0.17, 0.92, 0.14));
        let mano = mano + (puno - mano) * pose.prepara;
        let suelo = raiz.punto(Vec3::new(0.13, 0.07, 0.30));
        let mano = if lado > 0.0 { mano + (suelo - mano) * pose.azota } else { mano };
        let polo = torso.e[0] * lado * 0.8 - torso.e[1] * 1.3 + adelante * 0.25;
        let (l1, l2) = (0.23, 0.22);
        let c = codo(hombro, mano, l1, l2, polo);
        let hacia_codo = (c - hombro) / (c - hombro).norm().max(1e-4);
        let hacia_mano = (mano - c) / (mano - c).norm().max(1e-4);

        // Brazo: la manga corta verde y despues la camisa blanca.
        let medio = hombro + hacia_codo * (l1 * 0.45);
        v.push(segmento(hombro - hacia_codo * 0.03, medio, 0.125, 0.125, torso.e[0], M::Tunica));
        v.push(segmento(medio, c, 0.095, 0.095, torso.e[0], M::Blanco));
        // Antebrazo: camisa y el guantelete de cuero.
        let muneca = c + hacia_mano * (l2 * 0.4);
        v.push(segmento(c - hacia_mano * 0.02, muneca, 0.09, 0.09, torso.e[0], M::Blanco));
        v.push(segmento(muneca, mano - hacia_mano * 0.03, 0.105, 0.105, torso.e[0], M::Cuero));
        // La mano, envolviendo el extremo de la ocarina.
        v.push(Pieza {
            centro: mano,
            ejes: ocarina.e,
            tam: Vec3::new(0.06, 0.085, 0.085),
            mat: M::Piel,
            frente: None,
        });
    }

    cortes.push(v.len());
    // ---- EL ESCUDO HYLIANO ----
    // En la espalda, con el frente mirando hacia atras (girado media
    // vuelta) y apenas torcido. Dos cajas: la de arriba lleva el ave y la
    // Trifuerza, y un rombo detras hace la punta de abajo.
    let espalda = torso.hijo(Vec3::new(0.0, 0.24, -0.185), ejes_de(PI, -0.08, 0.10));
    let mut escudo = espalda.recta(Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.38, 0.34, 0.04), M::EscudoCanto);
    escudo.frente = Some(Frente::Escudo);
    v.push(escudo);
    let mut punta = espalda.caja(
        Vec3::new(0.0, -0.12, -0.004),
        Vec3::new(0.269, 0.269, 0.04),
        ejes_de(0.0, 0.0, PI / 4.0),
        M::EscudoCanto,
    );
    punta.frente = Some(Frente::EscudoPunta);
    v.push(punta);

    // ---- LA ESPADA MAESTRA ----
    // Enfundada en diagonal detras del escudo: la vaina asoma abajo a la
    // derecha y la empunadura arriba a la izquierda, sobre el hombro.
    let arriba = torso.punto(Vec3::new(-0.17, 0.52, -0.14));
    let abajo = torso.punto(Vec3::new(0.26, -0.18, -0.14));
    let eje_espada = (abajo - arriba) / (abajo - arriba).norm();
    let lado_espada = torso.e[2];
    v.push(segmento(arriba, abajo, 0.075, 0.05, lado_espada, M::Vaina));
    v.push(segmento(abajo - eje_espada * 0.06, abajo + eje_espada * 0.02, 0.085, 0.06, lado_espada, M::Oro));
    // La guarda: las alas azules, perpendiculares a la hoja.
    let guarda = arriba - eje_espada * 0.02;
    let ala = normalize(&cross(&eje_espada, &lado_espada));
    v.push(segmento(guarda - ala * 0.10, guarda + ala * 0.10, 0.045, 0.05, lado_espada, M::Empunadura));
    // El puno, violeta, y el pomo.
    let tope = arriba - eje_espada * 0.19;
    v.push(segmento(guarda, tope, 0.035, 0.035, lado_espada, M::Empunadura));
    v.push(segmento(tope, tope - eje_espada * 0.04, 0.05, 0.05, lado_espada, M::Oro));

    cortes.push(v.len());
    (v, cortes)
}

/// Link ya armado dentro de la escena: donde quedo su grupo y cual de sus
/// piezas es la ocarina, que es la que se enciende con las notas.
pub struct LinkVivo {
    pub grupo: usize,
    ocarina: usize,
}

/// Los materiales, en el orden de `M`.
fn materiales() -> (Vec<Material>, Material, Material, Material) {
    let tela = |base: [f32; 3], semilla: u32| {
        let ruido = campo_fbm(128, 6, 4, semilla);
        TextureImage::pintada(128, 128, move |u, v| {
            let n = ruido[((v * 127.0) as usize) * 128 + (u * 127.0) as usize];
            // La trama: un entrelazado fino de hilos que corre en las dos
            // direcciones. Es lo que hace que de cerca se lea TELA y no
            // plastico verde.
            let hilo = 0.93 + 0.07 * ((u * 128.0 * PI).sin() * (v * 128.0 * PI).sin()).abs();
            let k = (0.80 + 0.35 * n) * hilo;
            [base[0] * k, base[1] * k, base[2] * k]
        })
    };
    let img = |t: TextureImage| Texture::ImageTexture(Arc::new(t), Color::WHITE, (0.0, 0.0));

    // La tunica: el verde del juego, algo mas oscuro que el de la caja para
    // que bajo la luz rosa de la fuente siga leyendose verde.
    let tunica = Material::new(
        [1.0, 0.08, 0.0, 0.0],
        12.0,
        0.0,
        img(tela([0.20, 0.52, 0.16], 71)),
        None,
    );
    let piel_img = {
        let ruido = campo_fbm(64, 5, 3, 83);
        TextureImage::pintada(64, 64, move |u, v| {
            let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
            let k = 0.94 + 0.10 * n;
            [1.0 * k, 0.78 * k, 0.62 * k]
        })
    };
    let piel = Material::new([1.0, 0.18, 0.0, 0.0], 20.0, 0.0, img(piel_img), None);
    let pelo_img = {
        let ruido = campo_fbm(64, 4, 3, 97);
        TextureImage::pintada(64, 64, move |u, v| {
            let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
            // Mechones: vetas a lo largo, deformadas por el ruido.
            let veta = 0.78 + 0.22 * ((u * 40.0 + n * 6.0).sin() * 0.5 + 0.5);
            [1.0 * veta, 0.80 * veta, 0.30 * veta]
        })
    };
    let pelo = Material::new([1.0, 0.45, 0.0, 0.0], 40.0, 0.0, img(pelo_img), None);
    let blanco = Material::new([0.8, 0.05, 0.0, 0.0], 8.0, 0.0, img(tela([0.72, 0.70, 0.62], 113)), None);
    let cuero_img = |base: [f32; 3], semilla: u32| {
        let ruido = campo_fbm(64, 8, 4, semilla);
        TextureImage::pintada(64, 64, move |u, v| {
            let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
            // Arrugas: donde el ruido cruza por la mitad, una linea oscura.
            let arruga = 1.0 - 0.35 * (1.0 - ((n - 0.5).abs() * 18.0).min(1.0));
            let k = (0.75 + 0.45 * n) * arruga;
            [base[0] * k, base[1] * k, base[2] * k]
        })
    };
    let cuero = Material::new([1.0, 0.25, 0.02, 0.0], 25.0, 0.0, img(cuero_img([0.50, 0.30, 0.15], 131)), None);
    let cuero_oscuro =
        Material::new([1.0, 0.25, 0.02, 0.0], 25.0, 0.0, img(cuero_img([0.30, 0.17, 0.09], 137)), None);
    let oro = Material::new(
        [1.0, 0.9, 0.35, 0.0],
        90.0,
        0.0,
        Texture::Solid(Color::new(230, 180, 60, 255)),
        None,
    );
    // El canto y el dorso del escudo: acero.
    let acero = Material::new(
        [0.8, 0.8, 0.35, 0.0],
        70.0,
        0.0,
        Texture::Solid(Color::new(150, 155, 170, 255)),
        None,
    )
    .con_rugosidad(0.08);
    // LA OCARINA DEL TIEMPO: ceramica azul esmaltada. Muy brillante y con
    // reflejo, como el esmalte. La emision la enciende la animacion cuando
    // suena una nota.
    let ocarina = Material::new(
        [0.7, 1.0, 0.35, 0.0],
        140.0,
        0.0,
        Texture::Solid(Color::new(60, 120, 245, 255)),
        Some(Color::new(10, 25, 60, 255)),
    );
    let empunadura = Material::new(
        [0.9, 0.9, 0.3, 0.0],
        80.0,
        0.0,
        Texture::Solid(Color::new(95, 70, 200, 255)),
        None,
    );
    let vaina = Material::new(
        [1.0, 0.5, 0.1, 0.0],
        40.0,
        0.0,
        Texture::Solid(Color::new(40, 55, 120, 255)),
        None,
    );

    let cara = Material::new([1.0, 0.18, 0.0, 0.0], 20.0, 0.0, img(textura_cara()), None);
    let escudo = Material::new([1.0, 0.7, 0.25, 0.0], 60.0, 0.0, img(textura_escudo()), None)
        .con_rugosidad(0.10);
    let escudo_punta =
        Material::new([1.0, 0.7, 0.25, 0.0], 60.0, 0.0, img(textura_escudo_punta()), None).con_rugosidad(0.10);

    (
        vec![tunica, piel, pelo, blanco, cuero, cuero_oscuro, oro, acero, ocarina, empunadura, vaina],
        cara,
        escudo,
        escudo_punta,
    )
}

/// La cara de Link: ojos azules almendrados, cejas rubias, la boca.
fn textura_cara() -> TextureImage {
    TextureImage::pintada(128, 128, |u, v| {
        let piel = [1.0, 0.78, 0.62];
        // Sombreado hacia los costados: la cabeza es una caja, y sin esto
        // la cara se lee como una etiqueta pegada.
        let lado = 1.0 - 0.25 * ((u - 0.5) * 2.0).powi(4);
        let mut c = [piel[0] * lado, piel[1] * lado, piel[2] * lado];

        // Mejillas apenas rosadas.
        for cx in [0.24f32, 0.76] {
            let d = ((u - cx).powi(2) + (v - 0.68).powi(2)).sqrt();
            let k = (1.0 - d / 0.12).clamp(0.0, 1.0) * 0.18;
            c = [c[0], c[1] * (1.0 - k), c[2] * (1.0 - k * 0.8)];
        }

        // Los ojos: una almendra blanca con el iris azul y la pupila.
        for (cx, s) in [(0.30f32, -1.0f32), (0.70, 1.0)] {
            let dx = (u - cx) / 0.125;
            let dy = (v - 0.50) / 0.07;
            // Almendra: el borde de afuera sube un poco, como el ojo de Link.
            let dy = dy + dx * s * 0.25;
            let r = dx * dx + dy * dy;
            if r < 1.0 {
                c = [0.96, 0.96, 0.92];
                let (ix, iy) = ((u - cx + s * 0.012) / 0.05, (v - 0.505) / 0.058);
                let ri = ix * ix + iy * iy;
                if ri < 1.0 {
                    let k = 1.0 - 0.4 * ri;
                    c = [0.10 * k, 0.32 * k, 0.72 * k];
                    if ri < 0.25 {
                        c = [0.02, 0.03, 0.08];
                    }
                    let (bx, by) = ((u - cx + 0.015) / 0.012, (v - 0.49) / 0.012);
                    if bx * bx + by * by < 1.0 {
                        c = [1.0, 1.0, 1.0];
                    }
                }
            } else if r < 1.5 && dy < 0.3 {
                // El parpado de arriba, oscuro.
                c = [0.30, 0.18, 0.12];
            }
            // La ceja: una franja rubia inclinada sobre el ojo.
            let ex = (u - cx) / 0.13;
            let ey = (v - (0.36 - s * (u - cx) * 0.25)) / 0.022;
            if ex.abs() < 1.0 && ey.abs() < 1.0 {
                c = [0.78, 0.55, 0.20];
            }
        }

        // La nariz, una sombra, y la boca.
        if (u - 0.5).abs() < 0.03 && (v - 0.64).abs() < 0.03 {
            c = [c[0] * 0.85, c[1] * 0.82, c[2] * 0.82];
        }
        if (u - 0.5).abs() < 0.075 && (v - 0.79 - (u - 0.5).powi(2) * 3.0).abs() < 0.012 {
            c = [0.62, 0.30, 0.26];
        }
        c
    })
}

/// El frente del escudo hyliano: azul con canto de plata, la Trifuerza de
/// oro arriba y el ave roja de Hyrule con las alas abiertas.
fn textura_escudo() -> TextureImage {
    let ruido = campo_fbm(128, 6, 3, 151);
    TextureImage::pintada(128, 128, move |u, v| {
        let n = ruido[((v * 127.0) as usize) * 128 + (u * 127.0) as usize];
        let plata = [0.78, 0.80, 0.86];
        let azul = [0.10 + 0.04 * n, 0.20 + 0.05 * n, 0.55 + 0.10 * n];
        let rojo = [0.78, 0.08, 0.08];
        let oro = [1.0, 0.80, 0.25];

        // El canto: arriba y a los costados. Abajo no, que sigue en la punta.
        if u < 0.08 || u > 0.92 || v < 0.08 {
            let k = 0.85 + 0.15 * n;
            return [plata[0] * k, plata[1] * k, plata[2] * k];
        }
        let x = u - 0.5;

        // La Trifuerza: tres triangulos, el de arriba y los dos de abajo.
        let tri = |cx: f32, base: f32, alto: f32| {
            let t = (base - v) / alto;
            (0.0..1.0).contains(&t) && (u - cx).abs() < (1.0 - t) * alto * 0.577
        };
        let (a, y0) = (0.11f32, 0.40f32);
        if tri(0.5, y0 - a, a) || tri(0.5 - a * 0.577, y0, a) || tri(0.5 + a * 0.577, y0, a) {
            return oro;
        }

        // El ave: el cuerpo y la cabeza al centro, y las dos alas abiertas
        // en V hacia arriba, con plumas en el borde de abajo.
        let ax = x.abs();
        if ax < 0.035 && (0.47..1.0).contains(&v) {
            return rojo;
        }
        if ((ax / 0.05) + ((v - 0.47) / 0.05).abs()) < 1.0 {
            return rojo;
        }
        if (0.05..0.40).contains(&ax) {
            let s = (ax - 0.05) / 0.35;
            let arriba = 0.58 - 0.20 * s;
            let abajo = 0.70 - 0.12 * s + 0.035 * ((s * 16.0).sin()).abs();
            if v > arriba && v < abajo {
                return rojo;
            }
        }
        azul
    })
}

/// La punta del escudo: azul con canto de plata en los cuatro lados del
/// rombo (que son los dos que se ven) y el remate rojo del ave.
fn textura_escudo_punta() -> TextureImage {
    let ruido = campo_fbm(64, 5, 3, 157);
    TextureImage::pintada(64, 64, move |u, v| {
        let n = ruido[((v * 63.0) as usize) * 64 + (u * 63.0) as usize];
        let borde = u.min(v).min(1.0 - u).min(1.0 - v);
        if borde < 0.11 {
            let k = 0.85 + 0.15 * n;
            return [0.78 * k, 0.80 * k, 0.86 * k];
        }
        // El rombo esta girado 45 grados: la vertical del escudo es su
        // diagonal. El remate rojo es un rombito cerca del centro.
        let (a, b) = (u - 0.5, v - 0.5);
        if a.abs() + b.abs() < 0.14 {
            return [0.78, 0.08, 0.08];
        }
        [0.10 + 0.04 * n, 0.20 + 0.05 * n, 0.55 + 0.10 * n]
    })
}

/// Arma a Link y lo agrega a la escena. Devuelve lo que la animacion
/// necesita para moverlo.
pub fn armar(objetos: &mut Vec<Box<dyn RayIntersect + Send + Sync>>) -> LinkVivo {
    let (mats, cara, escudo, escudo_punta) = materiales();
    let (todas, cortes) = piezas(&Pose::quieta());
    let ocarina = todas.iter().position(|p| matches!(p.mat, M::Ocarina)).unwrap_or(usize::MAX);
    let mut cajas = todas.iter().map(|p| {
        let mut caja = CajaOrientada::nueva(p.centro, p.tam, p.ejes, mats[p.mat as usize].clone());
        if let Some(f) = p.frente {
            caja = caja.con_frente(match f {
                Frente::Cara => cara.clone(),
                Frente::Escudo => escudo.clone(),
                Frente::EscudoPunta => escudo_punta.clone(),
            });
        }
        Box::new(caja) as Box<dyn RayIntersect + Send + Sync>
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
    // El margen cubre lo que se mueve: el balanceo y el gorro.
    objetos.push(Box::new(GrupoAcotado::con_margen(partes, 0.25)));
    LinkVivo { grupo, ocarina }
}

impl LinkVivo {
    /// Posa a Link para este instante de la cancion.
    pub fn actualizar(&self, objetos: &mut [Box<dyn RayIntersect + Send + Sync>], p: &SceneParams, nota: f32) {
        let pose = pose_de(p);
        let (nuevas, _) = piezas(&pose);
        let Some(objeto) = objetos.get_mut(self.grupo) else {
            return;
        };
        let Some(grupo) = (objeto.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>() else {
            return;
        };
        let mut i = 0;
        for parte in grupo.children_mut() {
            let Some(parte) = (parte.as_mut() as &mut dyn Any).downcast_mut::<GrupoAcotado>() else {
                continue;
            };
            for hijo in parte.children_mut() {
                let Some(pieza) = nuevas.get(i) else { break };
                if let Some(caja) = (hijo.as_mut() as &mut dyn Any).downcast_mut::<CajaOrientada>() {
                    caja.colocar(pieza.centro, pieza.ejes);
                    caja.medio = pieza.tam * 0.5;
                    if i == self.ocarina {
                        // La ocarina se enciende con lo que suena.
                        let k = nota.clamp(0.0, 1.0);
                        let c = |base: f32, tope: f32| (base + (tope - base) * k) as u8;
                        caja.material.emission_color =
                            Some(Color::new(c(10.0, 60.0), c(25.0, 140.0), c(60.0, 255.0), 255));
                    }
                }
                i += 1;
            }
            parte.recalcular_caja(0.01);
        }
        grupo.recalcular_caja(0.01);
    }

    /// Donde esta la cabeza de Link ahora: el centro de la orbita de Navi.
    pub fn cabeza(&self, p: &SceneParams) -> Vec3 {
        let e = Esqueleto::de(&pose_de(p));
        e.cabeza.punto(Vec3::new(0.0, 0.15, 0.0))
    }

    /// Donde esta la ocarina ahora: de ahi salen las notas.
    pub fn ocarina(&self, p: &SceneParams) -> Vec3 {
        Esqueleto::de(&pose_de(p)).cabeza.punto(OCARINA_EN_CABEZA)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El codo deja cada tramo del brazo con su largo, y dobla hacia el polo.
    #[test]
    fn el_codo_respeta_los_largos_y_el_polo() {
        let (s, h) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.3, 0.1, 0.2));
        let polo = Vec3::new(0.0, -1.0, 0.0);
        let c = codo(s, h, 0.23, 0.22, polo);
        assert!(((c - s).norm() - 0.23).abs() < 1e-3, "brazo: {}", (c - s).norm());
        assert!(((h - c).norm() - 0.22).abs() < 1e-3, "antebrazo: {}", (h - c).norm());
        assert!(c.y < (s.y + h.y) * 0.5, "el codo no bajo hacia el polo");
    }

    /// Si la mano queda fuera de alcance el brazo se estira derecho hacia
    /// ella en vez de romperse.
    #[test]
    fn fuera_de_alcance_el_brazo_se_estira() {
        let c = codo(Vec3::zeros(), Vec3::new(2.0, 0.0, 0.0), 0.23, 0.22, Vec3::new(0.0, -1.0, 0.0));
        assert!((c.x - 0.23).abs() < 0.02 && c.y.abs() < 0.03, "codo en {c:?}");
    }

    /// Las manos quedan sobre la ocarina aunque Link se meza y asienta.
    #[test]
    fn las_manos_siguen_a_la_ocarina() {
        for (balanceo, asiente) in [(0.0f32, 0.0f32), (0.08, 0.07), (-0.08, -0.02)] {
            let pose = Pose { balanceo, asiente, ..Pose::quieta() };
            let (todas, _) = piezas(&pose);
            let boca = Esqueleto::de(&pose).cabeza.punto(OCARINA_EN_CABEZA);
            let manos: Vec<&Pieza> = todas
                .iter()
                .filter(|p| matches!(p.mat, M::Piel) && (p.tam.x - 0.06).abs() < 1e-4)
                .collect();
            assert_eq!(manos.len(), 2);
            for m in manos {
                assert!((m.centro - boca).norm() < 0.12, "una mano quedo lejos de la ocarina");
            }
        }
    }
}
